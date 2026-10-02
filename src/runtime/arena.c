#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#define DEFAULT_CHUNK_CAPACITY (64 * 1024)

typedef struct ArenaChunk {
    uint8_t* data;
    size_t capacity;
    size_t offset;
    struct ArenaChunk* next;
} ArenaChunk;

typedef struct Arena {
    ArenaChunk* head;
    ArenaChunk* current;
    size_t default_chunk_size;
    size_t total_allocated;
    size_t peak_usage;
} Arena;

static Arena* g_default_arena = NULL;

static ArenaChunk* alloc_chunk(size_t capacity) {
    ArenaChunk* chunk = (ArenaChunk*)malloc(sizeof(ArenaChunk));
    if (!chunk) return NULL;
    chunk->data = (uint8_t*)malloc(capacity);
    if (!chunk->data) {
        free(chunk);
        return NULL;
    }
    chunk->capacity = capacity;
    chunk->offset = 0;
    chunk->next = NULL;
    return chunk;
}

Arena* __nl_arena_create(size_t capacity) {
    if (capacity == 0) capacity = DEFAULT_CHUNK_CAPACITY;
    Arena* arena = (Arena*)malloc(sizeof(Arena));
    if (!arena) return NULL;
    ArenaChunk* chunk = alloc_chunk(capacity);
    if (!chunk) {
        free(arena);
        return NULL;
    }
    arena->head = chunk;
    arena->current = chunk;
    arena->default_chunk_size = capacity;
    arena->total_allocated = 0;
    arena->peak_usage = 0;
    return arena;
}

uint8_t* __nl_arena_alloc(Arena* arena, size_t size) {
    if (!arena || size == 0) return NULL;
    size_t aligned_size = (size + 7) & ~((size_t)7);
    ArenaChunk* curr = arena->current;
    if (curr->offset + aligned_size <= curr->capacity) {
        uint8_t* res = curr->data + curr->offset;
        curr->offset += aligned_size;
        arena->total_allocated += aligned_size;
        if (arena->total_allocated > arena->peak_usage) {
            arena->peak_usage = arena->total_allocated;
        }
        return res;
    }
    if (curr->next && curr->next->capacity >= aligned_size) {
        arena->current = curr->next;
        ArenaChunk* c = arena->current;
        c->offset = aligned_size;
        arena->total_allocated += aligned_size;
        if (arena->total_allocated > arena->peak_usage) {
            arena->peak_usage = arena->total_allocated;
        }
        return c->data;
    }
    size_t new_cap = arena->default_chunk_size > (aligned_size * 2) ? arena->default_chunk_size : (aligned_size * 2);
    ArenaChunk* new_chunk = alloc_chunk(new_cap);
    if (!new_chunk) return NULL;
    curr->next = new_chunk;
    arena->current = new_chunk;
    new_chunk->offset = aligned_size;
    arena->total_allocated += aligned_size;
    if (arena->total_allocated > arena->peak_usage) {
        arena->peak_usage = arena->total_allocated;
    }
    return new_chunk->data;
}

void __nl_arena_reset(Arena* arena) {
    if (!arena) return;
    ArenaChunk* curr = arena->head;
    while (curr) {
        curr->offset = 0;
        curr = curr->next;
    }
    arena->current = arena->head;
    arena->total_allocated = 0;
}

void __nl_arena_destroy(Arena* arena) {
    if (!arena) return;
    ArenaChunk* curr = arena->head;
    while (curr) {
        ArenaChunk* next = curr->next;
        if (curr->data) free(curr->data);
        free(curr);
        curr = next;
    }
    free(arena);
}

static void ensure_default_arena(void) {
    if (!g_default_arena) {
        g_default_arena = __nl_arena_create(DEFAULT_CHUNK_CAPACITY);
    }
}

uint8_t* __nl_arena_alloc_default(size_t size) {
    ensure_default_arena();
    return __nl_arena_alloc(g_default_arena, size);
}

void __nl_loop_reset(void) {
    if (g_default_arena) {
        __nl_arena_reset(g_default_arena);
    }
}

size_t __nl_arena_get_allocated_bytes(void) {
    return g_default_arena ? g_default_arena->total_allocated : 0;
}

size_t __nl_arena_get_peak_bytes(void) {
    return g_default_arena ? g_default_arena->peak_usage : 0;
}
