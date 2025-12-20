use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use ultimate_tic_tac_toe::{board::Board, types::Player};

fn criterion_benchmark(c: &mut Criterion) {
    let empty = Board::default();
    c.bench_function("move scores on empty board", |b| {
        b.iter(|| empty.find_best_move_score(black_box(Player::Player1)))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
