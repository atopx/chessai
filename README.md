# ChessAI

[![Crates.io](https://img.shields.io/crates/v/chessai.svg)](https://crates.io/crates/chessai)
[![docs.rs](https://img.shields.io/docsrs/chessai)](https://docs.rs/chessai)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

高性能中国象棋（Xiangqi）AI 引擎，基于 `u128` 位棋盘（bitboard）的纯 Rust 实现。

## 特性

- **位棋盘表示**：单个 `u128` 覆盖 10×9 的中国象棋棋盘，所有走法生成均为位运算。
- **Magic Bitboard 攻击表**：车、炮的快速查表攻击生成；马、象、兵、士、将采用预生成表。
- **Alpha-Beta 搜索**：迭代加深 + 换位表（Zobrist 键）+ 空着裁剪 + PVS + 静态搜索（QS）。
- **走法排序**：杀手启发、历史启发、反制走法、MVV-LVA、SEE 裁剪。
- **Lazy SMP 并行**：多线程共享换位表，配合 depth-skip 模式分散搜索。
- **开局库**：内嵌 `assets/BOOK.DAT`，支持走法镜像。
- **FEN & ICCS**：完整的 FEN 解析/生成，ICCS 坐标（`b2-e2` 或 `b2e2`）双向转换。
- **构建器 API**：`Engine::builder().hash_size(mb).threads(n).build()`，零配置即可运行。

## 安装

```toml
[dependencies]
chessai = "1"
```

## 快速开始

```rust
use std::time::Duration;
use chessai::{Engine, Limits};

let mut engine = Engine::builder()
    .hash_size(128)   // 换位表 128 MB
    .threads(4)       // Lazy SMP 4 线程
    .build();

let info = engine.search(
    Limits::new()
        .depth(12)
        .time(Duration::from_millis(500)),
);

if let Some(mv) = info.best_move {
    println!("best={mv} score={} depth={} nodes={} nps={} score={}",
        info.score, info.depth, info.nodes, info.nps, info.score);
}
```

### 从任意 FEN 开始

```rust
use chessai::Engine;

let mut engine = Engine::builder().build();
engine.set_fen("rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w")?;
println!("fen: {}", engine.fen());
println!("legal moves: {}", engine.legal_moves().len());
# Ok::<(), chessai::ChessAIError>(())
```

### 走子与对弈循环

```rust
use chessai::{Engine, Limits, Move};
use std::time::Duration;

let mut engine = Engine::builder().build();
let mv = Move::from_iccs("b2-e2")?;
assert!(engine.make_move(mv));

let reply = engine.search(Limits::new().time(Duration::from_millis(200)));
println!("engine plays: {:?}", reply.best_move);
# Ok::<(), chessai::ChessAIError>(())
```

### 迭代回调

`search_with` 在每个完成的迭代深度触发一次回调，便于向 UI/日志流式输出：

```rust
use chessai::{Engine, Limits};
use std::time::Duration;

let mut engine = Engine::builder().threads(2).build();
engine.search_with(
    Limits::new().depth(14).time(Duration::from_secs(2)),
    |info| println!("d={} score={} pv={:?}", info.depth, info.score, info.pv),
);
```

### 跨线程停止

`stop_handle()` 返回一个 `Arc<AtomicBool>`，任意线程写 `true` 即可请求搜索尽早返回当前最佳结果。

```rust
use chessai::{Engine, Limits};
use std::sync::atomic::Ordering;
use std::time::Duration;

let mut engine = Engine::builder().build();
let stop = engine.stop_handle();

// 在其他线程按 GUI 按钮时：
// stop.store(true, Ordering::Relaxed);

let info = engine.search(Limits::new().time(Duration::from_secs(5)));
# let _ = (stop, info);
```

## 公共 API

| 类型 | 说明 |
|------|------|
| `Engine` / `EngineBuilder` | 引擎主入口，搜索与状态管理 |
| `Position` | 不可变棋局视图（通过 `engine.position()` 获取） |
| `Move` | 16 位压缩走法，支持 ICCS `from_iccs` / `to_iccs` |
| `Square` | 0..=89 的格子索引，支持 ICCS (`a0..i9`) |
| `Piece` / `PieceType` | 带颜色的棋子与棋子种类 |
| `Color` | `Red` / `Black` |
| `Limits` | 搜索限制（深度、时间、节点） |
| `SearchInfo` | 搜索结果快照（best_move、pv、score、nodes、nps、time） |
| `ChessAIError` | 统一错误类型（FEN / ICCS 解析错误） |

### `Engine` 常用方法

- `Engine::builder() -> EngineBuilder` — `hash_size(mb)`、`threads(n)`、`use_book(bool)`、`build()`
- `engine.set_fen(&str) -> Result<(), ChessAIError>` — 加载 FEN，自动清空 TT 与历史
- `engine.reset_to_startpos()` — 复位到开局
- `engine.fen() -> String` — 导出当前 FEN
- `engine.side_to_move() -> Color`
- `engine.legal_moves() -> Vec<Move>`
- `engine.make_move(Move) -> bool` — 伪合法校验 + 将军校验
- `engine.book_move() -> Option<Move>` — 探询开局库
- `engine.search(Limits) -> SearchInfo`
- `engine.search_with(Limits, |&SearchInfo| …) -> SearchInfo`
- `engine.stop_handle() -> Arc<AtomicBool>`

## 项目结构

```text
chessai/
├── Cargo.toml
├── README.md
├── LICENSE
├── assets/
│   └── BOOK.DAT          # 内嵌开局库
└── src/
    ├── lib.rs            # 公共导出
    ├── engine.rs         # Engine / EngineBuilder
    ├── position.rs       # 棋局状态、make/undo、Zobrist 增量更新
    ├── movegen.rs        # 伪合法走法 / captures / quiets 生成
    ├── attacks.rs        # 马、象、兵、士、将的攻击表
    ├── magic.rs          # 车、炮的 Magic Bitboard 查表
    ├── bitboard.rs       # u128 位棋盘原语与 90 格掩码
    ├── search.rs         # Alpha-Beta、QS、迭代加深、Lazy SMP
    ├── picker.rs         # 分阶段走法挑选器
    ├── see.rs            # 静态交换评估
    ├── eval.rs           # 物质 + PST 增量评估
    ├── tt.rs             # 换位表（Zobrist 键 + lock 校验）
    ├── zobrist.rs        # Zobrist 随机键
    ├── book.rs           # 开局库探询
    ├── fen.rs            # FEN 解析/生成
    ├── limits.rs         # 搜索限制
    ├── mv.rs             # 走法压缩表示
    ├── square.rs         # 格子索引与 ICCS
    ├── piece.rs / color.rs
    ├── util.rs           # SplitMix64 RNG
    └── error.rs          # ChessAIError
```

## 性能优化

- **搜索**：PVS、NMP（空着裁剪）、LMR（后续走法减少）、Futility、SEE 裁剪
- **走法排序**：TT 走法 → captures(MVV-LVA) → killers → countermove → history
- **换位表**：Zobrist u64 键 + 32 位 lock 校验；置换策略按深度/年龄择优
- **Lazy SMP**：主线程 id=0 驱动回调，工作线程按 Stockfish 风格 SKIP_SIZE/SKIP_PHASE 错开深度
- **增量评估**：`make_move`/`undo_move` 同步维护物质分与 PST 分，避免全盘重算

## 从源码构建

```bash
git clone https://github.com/atopx/chessai.git
cd chessai
cargo build --release
```
