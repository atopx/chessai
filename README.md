# ChessAI - 中国象棋人工智能

ChessAI是一个使用Rust开发的中国象棋人工智能引擎。本项目实现了完整的中国象棋规则、棋盘状态评估、搜索算法和开局库，可用于象棋对弈、分析和训练。

## 功能特点

- 完整实现中国象棋规则和逻辑
- 高效的棋盘状态表示和评估
- Alpha-Beta剪枝搜索算法
- 开局库支持
- FEN棋谱格式解析与生成
- 中国象棋传统记谱法(ICCS)支持

## 项目结构

```
src/
├── lib.rs        - 引擎核心实现
├── pregen.rs     - 预生成的常量和工具函数
├── util.rs       - 工具函数
├── book.rs       - 开局库实现
├── state.rs      - 状态管理
├── position.rs   - 位置表示和转换
└── data/         - 预计算数据
    ├── BOOK.dat          - 开局库数据
    ├── FORT.dat          - 棋盘九宫格数据
    ├── BROAD.dat         - 棋盘有效区域数据
    ├── KEY_TABLE.dat     - Zobrist哈希键表
    ├── LOCK_TABLE.dat    - Zobrist哈希锁表
    ├── PIECE_VALUE.dat   - 棋子价值数据
    ├── KNIGHT_PIN.dat    - 马腿位置数据
    └── LEGAL_SPAN.dat    - 合法移动范围数据
```

## 使用方法

### 作为库使用

```rust
# cargo add chessai
use chessai::Engine;

fn main() {
    // 创建引擎实例
    let mut engine = Engine::new();
    
    // 从FEN字符串加载棋盘
    engine.from_fen("rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w");
    
    // 进行搜索并获取最佳走法
    let best_move = engine.search_main(10, 5000); // 搜索深度10，超时5秒
    
    // 将走法转换为ICCS表示
    let iccs = chessai::position::move2iccs(best_move);
    println!("最佳走法: {}", iccs);
}
```

### 主要API

- `Engine::new()` - 创建新的引擎实例
- `Engine::from_fen(fen)` - 从FEN字符串加载棋盘状态
- `Engine::to_fen()` - 将当前棋盘状态导出为FEN字符串
- `Engine::search_main(depth, millis)` - 搜索最佳走法，指定深度和超时时间
- `Engine::make_move(mv)` - 执行走法
- `Engine::undo_make_move()` - 撤销走法
- `Engine::winner()` - 判断当前是否有赢家，返回胜方

## 性能

ChessAI使用了多种优化技术来提高搜索效率：

- Zobrist哈希表缓存
- Alpha-Beta剪枝
- 空着剪枝
- 历史启发式搜索
- 杀手着法表
- MVV/LVA启发排序

## 开发与贡献

欢迎对本项目进行贡献！可以通过提交Issue或Pull Request来参与项目开发。

## MIT许可证

[LICENSE](LICENSE)
