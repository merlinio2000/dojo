use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use ultimate_tic_tac_toe::{
    board::{Board, move_finder::BoardMoveFinder},
    tree::Tree,
    types::{Index, Player},
    util,
};

#[allow(unused)]
fn run_v1() {
    let mut board = Board::new();
    let mut input = String::new();
    let read_line_buffered = |buf: &mut String| {
        buf.clear();
        std::io::stdin().read_line(buf).unwrap();
    };
    let mut my_player = Player::Player2;

    let move_calc = &mut BoardMoveFinder::default();

    loop {
        read_line_buffered(&mut input);
        let (opp_row, opp_col) = input
            .trim_end()
            .split_once(' ')
            .expect("opponent input should have a space");
        let (opp_row, opp_col) = (
            opp_row.parse::<i32>().expect("opp_row is not usize"),
            opp_col.parse::<i32>().expect("opp_col is not usize"),
        );

        // read and discard available inputs
        read_line_buffered(&mut input);
        let n_available = input
            .trim_end()
            .parse::<usize>()
            .expect("n_available is not a usize");
        for _ in 0..n_available {
            read_line_buffered(&mut input);
        }

        if opp_row == -1 {
            my_player = Player::Player1;
        } else {
            board.set(opp_row as Index, opp_col as Index, my_player.other());
        }

        let (row, col) = Board::to_2d_idx(board.find_best_move(my_player, move_calc));
        board.set(row, col, my_player);
        println!("{row} {col}");
    }
}

fn spawn_stdin_channel() -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        loop {
            let mut buffer = String::new();
            std::io::stdin().read_line(&mut buffer).unwrap();
            tx.send(buffer).unwrap();
        }
    });
    rx
}

const TIMING_TOLERANCE: Duration = Duration::from_millis(3);
const FIRST_TURN_TIME: Duration = Duration::from_secs(1)
    .checked_sub(TIMING_TOLERANCE)
    .unwrap();
const TURN_TIME: Duration = Duration::from_millis(100)
    .checked_sub(TIMING_TOLERANCE)
    .unwrap();

fn run_v2() {
    let initial_start_time = Instant::now();
    let inital_end_time = initial_start_time + FIRST_TURN_TIME;
    let input_rx = spawn_stdin_channel();
    // try to be cheeky and calculate while the other person is doing their turn
    let calc_while_read_input = |tree: &mut Tree| {
        loop {
            match input_rx.try_recv() {
                Ok(input) => break input,
                Err(mpsc::TryRecvError::Empty) => {
                    tree.search_n(1);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    panic!("input channel closed")
                }
            }
        }
    };
    let mut tree = Tree::new();

    tree.search_until(inital_end_time);
    let mut best_move = tree.best_explored_move();

    loop {
        let input = calc_while_read_input(&mut tree);
        // read_line_buffered(&mut input);
        let (opp_row, opp_col) = input
            .trim_end()
            .split_once(' ')
            .expect("opponent input should have a space");
        let (opp_row, opp_col) = (
            opp_row.parse::<i32>().expect("opp_row is not i32"),
            opp_col.parse::<i32>().expect("opp_col is not i32"),
        );

        // read and discard available inputs
        let input = calc_while_read_input(&mut tree);
        let n_available = input
            .trim_end()
            .parse::<usize>()
            .expect("n_available is not a usize");
        for _ in 0..n_available {
            calc_while_read_input(&mut tree);
        }
        let turn_start = Instant::now();

        // -1 == initial turn (if ours)
        if opp_row >= 0 {
            let board_col_major_move = util::to_board_col_major_move(opp_row as u8, opp_col as u8);
            tree.apply_maybe_explored_move(board_col_major_move);
            tree.search_until(turn_start + TURN_TIME);
            best_move = tree.best_explored_move();
        }
        tree.apply_explored_move(best_move);

        let (row, col) = util::board_col_major_move_to_2d(best_move);

        println!("{row} {col}");
    }
}

fn main() {
    #[cfg(target_arch = "x86_64")]
    {
        assert!(std::is_x86_feature_detected!("bmi1"));
        assert!(std::is_x86_feature_detected!("bmi2"));
        assert!(std::is_x86_feature_detected!("popcnt"));
        assert!(std::is_x86_feature_detected!("avx"));
        assert!(std::is_x86_feature_detected!("avx2"));
    }

    run_v2();
}
