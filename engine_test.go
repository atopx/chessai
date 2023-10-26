package chessai_test

import (
	"fmt"
	"strings"
	"testing"

	"github.com/atopx/chessai"
)

func Test_UnsignedRightShift(t *testing.T) {
	fmt.Println(chessai.UnsignedRightShift(50343, 30) == 3)
}

func TestMoves(t *testing.T) {
	mvs := []string{
		"g3-g4",
		"g6-g5",
		"b0-c2",
		"h7-h0",
		"e3-e4",
		"d9-e8",
		"e1-e2",
		"c6-c5",
	}
	for _, mv := range mvs {
		mv = strings.ToUpper(mv)
		if mv != chessai.Move2Iccs(chessai.Iccs2Move(mv)) {
			t.Fatalf("error: %s", mv)
		}
	}
}

func Test_Search(t *testing.T) {
	eng := chessai.NewEngine()
	search := chessai.NewSearch(eng, 16)

	t.Run("26215", func(t *testing.T) {
		fen := "9/2Cca4/3k1C3/4P1p2/4N1b2/4R1r2/4c1n2/3p1n3/2rNK4/9 w"
		eng.FromFen(fen)
		mv := search.SearchMain(64, 1000)
		if mv != 26215 {
			t.Fatalf("error, excep: 26215: get %d", mv)
		}
	})

	t.Run("22326", func(t *testing.T) {
		fen := "C1nNk4/9/9/9/9/9/n1pp5/B3C4/9/3A1K3 w - - 0 1"
		eng.FromFen(fen)
		mv := search.SearchMain(64, 1000)
		if mv != 22326 {
			t.Fatalf("error, excep: 22326, get: 22326 %d(%s)", mv, chessai.Move2Iccs(mv))
		}
	})

	t.Run("22985", func(t *testing.T) {
		fen := "4kab2/4a4/8b/9/9/9/9/9/9/4K1R2 w - - 0 1"
		eng.FromFen(fen)
		mv := search.SearchMain(64, 1000)
		if mv != 22985 {
			t.Fatalf("error, excep: 22985, get: %d(%s)", mv, chessai.Move2Iccs(mv))
		}
	})
}

func Test_ShellSort(t *testing.T) {
	mvs := []int{22599, 34697, 30615, 34713, 46758, 34728, 46760, 13749, 46773}
	vls := []int{29, 36, 26, 39, 28, 39, 29, 26, 26}
	chessai.NewEngine().ShellSort(mvs, vls)
	expMvs := []int{34728, 34713, 34697, 22599, 46760, 46758, 30615, 13749, 46773}
	expVls := []int{39, 39, 36, 29, 29, 28, 26, 26, 26}
	for i := 0; i < 9; i++ {
		if expMvs[i] != mvs[i] {
			t.Fatalf("ShellSort error, \nexp mvs: %+v, \nget mvs: %+v\n", expMvs, mvs)
		}
		if vls[i] != expVls[i] {
			t.Fatalf("ShellSort error, \nexp vls: %+v, \nget vvs: %+v\n", expVls, vls)
		}
	}
}

func Benchmark_Search(b *testing.B) {
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		eng := chessai.NewEngine()
		search := chessai.NewSearch(eng, 16)
		fen := "9/2Cca4/3k1C3/4P1p2/4N1b2/4R1r2/4c1n2/3p1n3/2rNK4/9 w"
		eng.FromFen(fen)
		mv := search.SearchMain(64, 1000)
		if mv != 26215 {
			b.Fatalf("error, excep: 26215, get: %d(%s)", mv, chessai.Move2Iccs(mv))
		}
	}
}
