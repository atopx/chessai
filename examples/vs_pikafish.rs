//! Match ChessAI (Red) against Pikafish (Black) on the same board.
//!
//! Pikafish is driven over UCI on stdin/stdout; ChessAI uses the native
//! `Engine` API. Both sides are given the same per-move time budget. The game
//! ends on checkmate, stalemate, or when the ply cap is reached.
//!
//! Run from the project root so that the relative path to Pikafish resolves:
//!
//! ```sh
//! cargo run --release --example vs_pikafish
//! ```

use std::error::Error;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::process::Child;
use std::process::ChildStdin;
use std::process::ChildStdout;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;

use chessai::Color;
use chessai::Engine;
use chessai::Limits;
use chessai::Move;
use chessai::STARTING_FEN;

/// Working directory for the Pikafish subprocess. Pikafish loads `pikafish.nnue`
/// relative to its CWD, so both files live under `./pikafish/`.
const PIKAFISH_DIR: &str = "pikafish";
const PIKAFISH_BIN: &str = "./pikafish";

const MOVE_TIME_MS: u64 = 500;
const MAX_PLIES: usize = 300;

struct Pikafish {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Pikafish {
    fn spawn() -> std::io::Result<Self> {
        let mut child = Command::new(PIKAFISH_BIN)
            .current_dir(PIKAFISH_DIR)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = child.stdin.take().expect("stdin piped");
        let stdout = BufReader::new(child.stdout.take().expect("stdout piped"));
        let mut pika = Pikafish { child, stdin, stdout };
        pika.handshake()?;
        Ok(pika)
    }

    fn send(&mut self, line: &str) -> std::io::Result<()> {
        writeln!(self.stdin, "{line}")?;
        self.stdin.flush()
    }

    fn read_line(&mut self) -> std::io::Result<String> {
        let mut buf = String::new();
        let n = self.stdout.read_line(&mut buf)?;
        if n == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "pikafish closed stdout"));
        }
        Ok(buf.trim_end().to_string())
    }

    fn read_until(&mut self, needle: &str) -> std::io::Result<()> {
        loop {
            let line = self.read_line()?;
            if line.split_whitespace().next() == Some(needle) {
                return Ok(());
            }
        }
    }

    fn handshake(&mut self) -> std::io::Result<()> {
        self.send("uci")?;
        self.read_until("uciok")?;
        self.send("ucinewgame")?;
        self.send("isready")?;
        self.read_until("readyok")
    }

    fn ask_bestmove(&mut self, moves: &[String], movetime_ms: u64) -> std::io::Result<Option<String>> {
        let mut cmd = format!("position fen {STARTING_FEN}");
        if !moves.is_empty() {
            cmd.push_str(" moves ");
            cmd.push_str(&moves.join(" "));
        }
        self.send(&cmd)?;
        self.send(&format!("go movetime {movetime_ms}"))?;
        let mut last_line = String::new();
        loop {
            let line = self.read_line()?;
            if let Some(rest) = line.strip_prefix("bestmove ") {
                println!("{last_line}");
                let mv = rest.split_whitespace().next().unwrap_or("");
                if mv.is_empty() || mv == "(none)" || mv == "0000" {
                    return Ok(None);
                }
                return Ok(Some(mv.to_string()));
            }
            last_line = line
        }
    }

    fn quit(mut self) {
        let _ = self.send("quit");
        let _ = self.child.wait();
    }
}

fn move_to_uci(mv: Move) -> String {
    format!("{}{}", mv.src(), mv.dst())
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut engine = Engine::builder().hash_size(128).threads(2).use_book(true).build();
    let mut pika = Pikafish::spawn()?;
    let mut uci_history: Vec<String> = Vec::new();

    println!("ChessAI (Red) vs Pikafish (Black), {MOVE_TIME_MS}ms per move\n");

    for ply in 0..MAX_PLIES {
        let side = engine.side_to_move();
        let side_label = if side == Color::Red { "Red  (ChessAI) " } else { "Black (Pikafish)" };

        if engine.legal_moves().is_empty() {
            let verdict = if engine.position().is_in_check(side) {
                format!("checkmate — {side_label} wins")
            } else {
                "stalemate".to_string()
            };
            println!("\n=== Game over at ply {ply}: {verdict} ===");
            break;
        }
        println!("===================== Ply {ply} =====================");
        let mv = if side == Color::Red {
            let info = engine.search(Limits::new().time(Duration::from_millis(MOVE_TIME_MS * 2)));
            println!("ChessAI: {info:?}");
            info.best_move.ok_or("ChessAI returned no move despite legal moves existing")?
        } else {
            match pika.ask_bestmove(&uci_history, MOVE_TIME_MS)? {
                Some(uci) => Move::from_iccs(&uci)?,
                None => {
                    println!("\n=== Game over at ply {ply}: Pikafish resigned ===");
                    break;
                }
            }
        };

        let uci = move_to_uci(mv);
        if !engine.make_move(mv) {
            return Err(format!("illegal move from {side_label}: {uci}").into());
        }
        uci_history.push(uci.clone());

        // println!("Ply {:>3}  {side_label}  {uci}", ply + 1);
    }

    pika.quit();
    Ok(())
}
