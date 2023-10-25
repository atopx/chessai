mod engine;

fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    use crate::engine::ChessAi;

    #[test]
    fn test_engine() {
        let mut engine = ChessAi::new();
        engine.from_fen("9/2Cca4/3k1C3/4P1p2/4N1b2/4R1r2/4c1n2/3p1n3/2rNK4/9 w");
        let mv = engine.search_main(64, 1000);
        assert_eq!(mv, 26215);
    }
}
