use anyhow::{bail, Context, Result};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use axum_server::tls_rustls::RustlsConfig;
use futures_util::{SinkExt, StreamExt};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::Deserialize;
use std::{
    env,
    io::{Read, Write},
    net::ToSocketAddrs,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};
use tokio::sync::mpsc;

#[derive(Clone)]
struct AppState {
    token: Option<String>,
    exe_path: PathBuf,
    cwd: PathBuf,
}

#[derive(Deserialize)]
struct TokenQuery {
    token: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum ClientMessage {
    #[serde(rename = "input")]
    Input { data: String },
    #[serde(rename = "resize")]
    Resize { cols: u16, rows: u16 },
}

enum PtyInput {
    Data(Vec<u8>),
    Resize { cols: u16, rows: u16 },
}

struct PtySession {
    input_tx: std::sync::mpsc::Sender<PtyInput>,
    output_rx: mpsc::Receiver<Vec<u8>>,
    child: Box<dyn portable_pty::Child + Send>,
}

pub async fn run(
    host: &str,
    port: u16,
    token: Option<&str>,
    tls_cert: Option<&PathBuf>,
    tls_key: Option<&PathBuf>,
) -> Result<()> {
    let exe_path = env::current_exe().context("Failed to resolve current executable")?;
    let cwd = env::current_dir().context("Failed to resolve current directory")?;
    let state = AppState {
        token: token.map(|value| value.to_string()),
        exe_path,
        cwd,
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/ws", get(ws_handler))
        .with_state(state);

    let addr = resolve_bind_addr(host, port)?;
    let tls_config = match (tls_cert, tls_key) {
        (Some(cert), Some(key)) => Some(
            RustlsConfig::from_pem_file(cert, key)
                .await
                .context("Failed to load TLS certificate/key")?,
        ),
        (None, None) => None,
        _ => bail!("Both --tls-cert and --tls-key are required to enable TLS"),
    };
    print_urls(host, port, token, tls_config.is_some());

    if let Some(tls) = tls_config {
        axum_server::bind_rustls(addr, tls)
            .serve(app.into_make_service())
            .await
            .context("Failed to start HTTPS server")?;
    } else {
        axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .await
            .context("Failed to start web server")?;
    }

    Ok(())
}

fn resolve_bind_addr(host: &str, port: u16) -> Result<std::net::SocketAddr> {
    let addr = format!("{host}:{port}");
    let addr = addr
        .to_socket_addrs()
        .context("Failed to resolve bind address")?
        .next()
        .context("No bind address resolved")?;
    Ok(addr)
}

fn print_urls(host: &str, port: u16, token: Option<&str>, tls_enabled: bool) {
    let scheme = if tls_enabled { "https" } else { "http" };
    let suffix = token
        .map(|value| format!("?token={value}"))
        .unwrap_or_default();
    println!("Web TUI: {scheme}://{host}:{port}/{suffix}");
    if host == "0.0.0.0" || host == "::" {
        println!("Tip: use your machine's LAN IP on your phone (example: 192.168.x.x).");
    }
    if token.is_some() {
        println!("Token required for access.");
    }
}

async fn index(
    State(state): State<AppState>,
    Query(query): Query<TokenQuery>,
) -> impl IntoResponse {
    if !authorized(&state, query.token.as_deref()) {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }

    let token = state.token.as_deref().unwrap_or("");
    Html(page_html(token)).into_response()
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(query): Query<TokenQuery>,
) -> impl IntoResponse {
    if !authorized(&state, query.token.as_deref()) {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }

    ws.on_upgrade(move |socket| async move {
        if let Err(err) = handle_socket(socket, state).await {
            eprintln!("websocket error: {err}");
        }
    })
}

fn authorized(state: &AppState, token: Option<&str>) -> bool {
    match state.token.as_deref() {
        None => true,
        Some(expected) => token == Some(expected),
    }
}

async fn handle_socket(socket: WebSocket, state: AppState) -> Result<()> {
    let PtySession {
        input_tx,
        mut output_rx,
        mut child,
    } = spawn_pty(&state)?;

    let (mut ws_sender, mut ws_receiver) = socket.split();
    let output_task = tokio::spawn(async move {
        while let Some(chunk) = output_rx.recv().await {
            if ws_sender.send(Message::Binary(chunk)).await.is_err() {
                break;
            }
        }
    });

    while let Some(message) = ws_receiver.next().await {
        match message {
            Ok(Message::Text(text)) => {
                if let Ok(decoded) = serde_json::from_str::<ClientMessage>(&text) {
                    match decoded {
                        ClientMessage::Input { data } => {
                            let _ = input_tx.send(PtyInput::Data(data.into_bytes()));
                        }
                        ClientMessage::Resize { cols, rows } => {
                            let cols = cols.max(1);
                            let rows = rows.max(1);
                            let _ = input_tx.send(PtyInput::Resize { cols, rows });
                        }
                    }
                } else {
                    let _ = input_tx.send(PtyInput::Data(text.into_bytes()));
                }
            }
            Ok(Message::Binary(data)) => {
                let _ = input_tx.send(PtyInput::Data(data));
            }
            Ok(Message::Close(_)) | Err(_) => break,
            _ => {}
        }
    }

    drop(input_tx);
    let _ = child.kill();
    let _ = output_task.await;
    Ok(())
}

fn spawn_pty(state: &AppState) -> Result<PtySession> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("Failed to open PTY")?;

    let mut cmd = CommandBuilder::new(&state.exe_path);
    cmd.cwd(&state.cwd);
    cmd.env("TERM", "xterm-256color");

    let child = pair
        .slave
        .spawn_command(cmd)
        .context("Failed to spawn mycel TUI")?;

    let master = pair.master;
    let mut reader = master
        .try_clone_reader()
        .context("Failed to clone PTY reader")?;
    let mut writer = master.take_writer().context("Failed to take PTY writer")?;
    let master = Arc::new(Mutex::new(master));

    let (output_tx, output_rx) = mpsc::channel(32);
    thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if output_tx.blocking_send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let (input_tx, input_rx) = std::sync::mpsc::channel::<PtyInput>();
    let master_for_resize = Arc::clone(&master);
    thread::spawn(move || {
        while let Ok(message) = input_rx.recv() {
            match message {
                PtyInput::Data(data) => {
                    let _ = writer.write_all(&data);
                    let _ = writer.flush();
                }
                PtyInput::Resize { cols, rows } => {
                    let size = PtySize {
                        cols,
                        rows,
                        pixel_width: 0,
                        pixel_height: 0,
                    };
                    if let Ok(master) = master_for_resize.lock() {
                        let _ = master.resize(size);
                    }
                }
            }
        }
    });

    Ok(PtySession {
        input_tx,
        output_rx,
        child,
    })
}

fn page_html(token: &str) -> String {
    let token = escape_js_string(token);
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>mycel web</title>
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
  <link href="https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@400;600&display=swap" rel="stylesheet" />
  <link rel="stylesheet" href="https://unpkg.com/xterm@5.3.0/css/xterm.css" />
  <style>
    :root {{
      --bg: #0a1418;
      --bg-accent: #102530;
      --panel: rgba(6, 10, 12, 0.85);
      --accent: #38b49b;
      --text: #e6f2f0;
      --muted: #8aa4a8;
    }}
    * {{ box-sizing: border-box; }}
    body {{
      margin: 0;
      height: 100vh;
      color: var(--text);
      font-family: "Space Grotesk", "Segoe UI", sans-serif;
      background: radial-gradient(1200px 800px at 15% 10%, var(--bg-accent), var(--bg) 60%, #050709 100%);
    }}
    #shell {{
      display: flex;
      flex-direction: column;
      height: 100vh;
    }}
    #bar {{
      display: flex;
      align-items: center;
      gap: 10px;
      padding: 10px 14px;
      background: linear-gradient(90deg, rgba(9, 14, 18, 0.9), rgba(9, 16, 19, 0.55));
      border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    }}
    #dot {{
      width: 8px;
      height: 8px;
      border-radius: 999px;
      background: #f59e0b;
      box-shadow: 0 0 12px rgba(245, 158, 11, 0.6);
    }}
    #status {{
      font-size: 12px;
      letter-spacing: 0.12em;
      text-transform: uppercase;
      color: var(--muted);
    }}
    #hint {{
      margin-left: auto;
      font-size: 12px;
      color: var(--muted);
    }}
    #focus {{
      border: 1px solid rgba(56, 180, 155, 0.4);
      background: rgba(56, 180, 155, 0.15);
      color: var(--text);
      padding: 6px 10px;
      border-radius: 999px;
      font-family: inherit;
      font-size: 12px;
      cursor: pointer;
    }}
    #terminal {{
      flex: 1;
      background: var(--panel);
      touch-action: manipulation;
    }}
    .xterm {{
      padding: 8px;
    }}
    #keys {{
      display: none;
      gap: 8px;
      flex-wrap: wrap;
      padding: 10px 12px;
      background: linear-gradient(90deg, rgba(9, 16, 19, 0.65), rgba(9, 14, 18, 0.9));
      border-top: 1px solid rgba(255, 255, 255, 0.06);
    }}
    #keys button {{
      border: 1px solid rgba(255, 255, 255, 0.08);
      background: rgba(7, 12, 14, 0.7);
      color: var(--text);
      padding: 8px 12px;
      border-radius: 999px;
      font-family: inherit;
      font-size: 12px;
      letter-spacing: 0.08em;
      text-transform: uppercase;
      cursor: pointer;
    }}
    #keys button:active {{
      transform: translateY(1px);
      background: rgba(56, 180, 155, 0.2);
      border-color: rgba(56, 180, 155, 0.5);
    }}
    @media (pointer: coarse), (max-width: 720px) {{
      #keys {{
        display: flex;
      }}
      #hint {{
        display: none;
      }}
    }}
  </style>
</head>
<body>
  <div id="shell">
    <div id="bar">
      <div id="dot"></div>
      <div id="status">connecting</div>
      <button id="focus" type="button">focus</button>
      <div id="hint">tap terminal to type</div>
    </div>
    <div id="terminal"></div>
    <div id="keys" aria-label="mobile keys">
      <button type="button" data-key="esc">esc</button>
      <button type="button" data-key="tab">tab</button>
      <button type="button" data-key="ctrl_c">ctrl+c</button>
      <button type="button" data-key="up">up</button>
      <button type="button" data-key="down">down</button>
      <button type="button" data-key="left">left</button>
      <button type="button" data-key="right">right</button>
      <button type="button" data-key="enter">enter</button>
    </div>
  </div>
  <script src="https://unpkg.com/xterm@5.3.0/lib/xterm.js"></script>
  <script src="https://unpkg.com/xterm-addon-fit@0.8.0/lib/xterm-addon-fit.js"></script>
  <script>
    const TOKEN = '{token}';
    const term = new Terminal({{
      cursorBlink: true,
      fontFamily: 'JetBrains Mono, Menlo, Monaco, Consolas, monospace',
      fontSize: 14,
      scrollback: 2000,
      theme: {{
        background: '#0b1012',
        foreground: '#e6f2f0'
      }}
    }});
    const fitAddon = new FitAddon.FitAddon();
    term.loadAddon(fitAddon);
    term.open(document.getElementById('terminal'));
    fitAddon.fit();
    term.focus();
    if (term.textarea) {{
      term.textarea.setAttribute('inputmode', 'text');
      term.textarea.setAttribute('autocorrect', 'off');
      term.textarea.setAttribute('autocapitalize', 'off');
      term.textarea.setAttribute('spellcheck', 'false');
    }}

    const statusEl = document.getElementById('status');
    const dot = document.getElementById('dot');
    const focusBtn = document.getElementById('focus');
    const terminalEl = document.getElementById('terminal');
    const focusTerminal = () => term.focus();
    focusBtn.addEventListener('click', focusTerminal);
    focusBtn.addEventListener('touchstart', focusTerminal, {{ passive: true }});
    terminalEl.addEventListener('click', focusTerminal);
    terminalEl.addEventListener('touchstart', focusTerminal, {{ passive: true }});

    function updateStatus(text, color) {{
      statusEl.textContent = text;
      dot.style.background = color;
      dot.style.boxShadow = '0 0 12px ' + color;
    }}

    const wsUrl = new URL((location.protocol === 'https:' ? 'wss' : 'ws') + '://' + location.host + '/ws');
    if (TOKEN) {{
      wsUrl.searchParams.set('token', TOKEN);
    }}
    const ws = new WebSocket(wsUrl);
    ws.binaryType = 'arraybuffer';

    ws.onopen = () => {{
      updateStatus('connected', '#38b49b');
      sendResize();
    }};
    ws.onclose = () => {{
      updateStatus('disconnected', '#ef4444');
    }};
    ws.onerror = () => {{
      updateStatus('error', '#ef4444');
    }};
    ws.onmessage = (event) => {{
      if (typeof event.data === 'string') {{
        term.write(event.data);
      }} else {{
        term.write(new Uint8Array(event.data));
      }}
    }};

    function sendInput(data) {{
      if (ws.readyState !== WebSocket.OPEN) {{
        return;
      }}
      ws.send(JSON.stringify({{ type: 'input', data }}));
    }}
    term.onData((data) => {{
      sendInput(data);
    }});

    let resizeTimer = null;
    function sendResize() {{
      if (ws.readyState !== WebSocket.OPEN) {{
        return;
      }}
      ws.send(JSON.stringify({{ type: 'resize', cols: term.cols, rows: term.rows }}));
    }}
    function resizeTerminal() {{
      fitAddon.fit();
      sendResize();
    }}
    window.addEventListener('resize', () => {{
      clearTimeout(resizeTimer);
      resizeTimer = setTimeout(resizeTerminal, 150);
    }});
    const keySequences = {{
      esc: '\x1b',
      tab: '\t',
      ctrl_c: '\x03',
      up: '\x1b[A',
      down: '\x1b[B',
      right: '\x1b[C',
      left: '\x1b[D',
      enter: '\r'
    }};
    const keyButtons = document.querySelectorAll('#keys [data-key]');
    function handleKeyPress(event) {{
      event.preventDefault();
      const key = event.currentTarget.getAttribute('data-key');
      const sequence = keySequences[key];
      if (sequence) {{
        sendInput(sequence);
        focusTerminal();
      }}
    }}
    keyButtons.forEach((button) => {{
      button.addEventListener('click', handleKeyPress);
      button.addEventListener('touchstart', handleKeyPress, {{ passive: false }});
    }});
  </script>
</body>
</html>"#
    )
}

fn escape_js_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}
