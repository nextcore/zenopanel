package engine

import (
	"testing"
)

func TestArenaBasic(t *testing.T) {
	arena := NewArena(1024)

	// Test allocation alignment
	ptr1 := arena.Alloc(1)
	if uintptr(ptr1)%8 != 0 {
		t.Errorf("Expected 8-byte alignment for first allocation, got pointer %v", ptr1)
	}

	ptr2 := arena.Alloc(1)
	if uintptr(ptr2)%8 != 0 {
		t.Errorf("Expected 8-byte alignment for second allocation, got pointer %v", ptr2)
	}

	// Test Reset
	arena.Reset()
	if arena.offset != 0 {
		t.Errorf("Expected offset 0 after Reset, got %d", arena.offset)
	}
}

func TestArenaGrowth(t *testing.T) {
	arena := NewArena(10) // Very small

	_ = arena.Alloc(5)
	_ = arena.Alloc(10) // Should trigger growth

	used, total := arena.Stats()
	if total <= 10 {
		t.Errorf("Arena should have grown, but total capacity is still %d", total)
	}
	if used <= 15 {
		t.Errorf("Arena used space should be at least 15, got %d", used)
	}
}

func TestArenaAllocScope(t *testing.T) {
	arena := NewArena(1024)
	parent := NewScope(nil)
	parent.Set("global", 100)

	scope := arena.AllocScope(parent)
	if scope.parent != parent {
		t.Error("Scope parent not set correctly")
	}

	scope.Set("local", 200)
	val, ok := scope.Get("global")
	if !ok || val != 100 {
		t.Errorf("Expected 100 from parent, got %v", val)
	}

	val2, ok := scope.Get("local")
	if !ok || val2 != 200 {
		t.Errorf("Expected 200 from local, got %v", val2)
	}
}

func TestArenaAllocBuffer(t *testing.T) {
	arena := NewArena(100)
	buf := arena.AllocBuffer(50)

	if len(buf) != 50 {
		t.Errorf("Expected buffer length 50, got %d", len(buf))
	}
	if cap(buf) != 50 {
		t.Errorf("Expected buffer capacity 50, got %d", cap(buf))
	}

	copy(buf, []byte("hello arena"))

	// Check if data persists after another allocation
	_ = arena.Alloc(10)
	if string(buf[:11]) != "hello arena" {
		t.Errorf("Buffer data corrupted, got %s", string(buf[:11]))
	}
}

func BenchmarkArenaAlloc(b *testing.B) {
	arena := NewArena(1024 * 1024)
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		arena.Reset()
		for j := 0; j < 100; j++ {
			_ = arena.Alloc(16)
		}
	}
}

func BenchmarkHeapAlloc(b *testing.B) {
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		for j := 0; j < 100; j++ {
			_ = make([]byte, 16)
		}
	}
}
