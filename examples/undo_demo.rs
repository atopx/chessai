//! Demonstrate the make/undo loop on the public Engine API.
//!
//! Plays a few moves, walks them all back, and asserts the engine returns to
//! the starting FEN. Useful as a sanity check when integrating the lib into a
//! GUI / UCI adapter where take-back is required.
//!
//! ```sh
//! cargo run --release --example undo_demo
//! ```

use chessai::Engine;

fn main() {
    let mut engine = Engine::builder().use_book(false).build();
    let fen0 = engine.fen();

    println!("Start: {fen0}");
    println!("History len: {}\n", engine.history_len());

    let mut played = Vec::new();
    for ply in 0..5 {
        let mv = engine.legal_moves()[0];
        assert!(engine.make_move(mv), "move must be legal");
        played.push(mv);
        println!("Ply {ply}: played {} → fen={}", mv.to_iccs(), engine.fen());
    }

    println!("\nHistory len after 5 moves: {}", engine.history_len());
    let recorded: Vec<_> = engine.move_history().map(|m| m.to_iccs()).collect();
    println!("Recorded history: {recorded:?}\n");

    while let Some(mv) = engine.undo_move() {
        println!("Undo: {} (history_len={})", mv.to_iccs(), engine.history_len());
    }

    assert_eq!(engine.fen(), fen0, "FEN must match starting position after full unwind");
    assert_eq!(engine.history_len(), 0);
    assert_eq!(engine.undo_move(), None, "further undo on empty history yields None");

    println!("\nUnwound back to start. FEN matches: {}", engine.fen() == fen0);
}
