use std::{
    io::BufRead,
    sync::mpsc::{self},
    thread,
    time::{Duration, Instant},
};

use ultimate_tic_tac_toe::{
    board::{Board, move_finder::BoardMoveFinder},
    tree::{TreeForPlayer, TreePlayer1, TreePlayer2},
    types::{Index, PLAYER1_U8, PLAYER2_U8, Player, PlayerU8},
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
        let mut stdin_lock = std::io::stdin().lock();
        loop {
            let mut buffer = String::new();
            stdin_lock.read_line(&mut buffer).unwrap();
            tx.send(buffer).unwrap();
        }
    });
    rx
}

const TIMING_TOLERANCE: Duration = Duration::from_millis(20);
const FIRST_TURN_TIME: Duration = Duration::from_secs(1)
    .checked_sub(TIMING_TOLERANCE)
    .unwrap();
const TURN_TIME: Duration = Duration::from_millis(100)
    .checked_sub(TIMING_TOLERANCE)
    .unwrap();

fn read_and_ignore_available(input_rx: &mpsc::Receiver<String>) {
    // read and discard available inputs
    let input = input_rx.recv().expect("failed to read n_available");
    // <= 81
    let n_available = input
        .trim_end()
        .parse::<u8>()
        .expect("n_available is not an u8");
    for _ in 0..n_available {
        input_rx.recv().expect("failed to read available move");
    }
}

fn run_v2_on_initialized_tree<const SCORE_IN_FAVOR_OF: PlayerU8>(
    mut tree: TreeForPlayer<SCORE_IN_FAVOR_OF>,
    input_rx: mpsc::Receiver<String>,
) {
    // try to be cheeky and calculate while the other person is doing their turn
    #[expect(unused)]
    let calc_while_read_input = |tree: &mut TreeForPlayer<SCORE_IN_FAVOR_OF>| {
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
    let mut best_move = tree.best_explored_move();
    tree.apply_explored_move(best_move);

    let (row, col) = util::board_col_major_move_to_2d(best_move);
    println!("{row} {col}");

    loop {
        // TODO MERBUG: re-enable cheeky calcing but fix perspective
        // let input = calc_while_read_input(&mut tree);
        let input = input_rx.recv().expect("failed to read opponent move");
        let (opp_row, opp_col) = input
            .trim_end()
            .split_once(' ')
            .expect("opponent input should have a space {input:?}");
        let (opp_row, opp_col) = (
            opp_row.parse::<u8>().expect("opp_row is not u8"),
            opp_col.parse::<u8>().expect("opp_col is not u8"),
        );

        read_and_ignore_available(&input_rx);
        let turn_start = Instant::now();

        let opp_board_col_major_move = util::to_board_col_major_move(opp_row, opp_col);
        tree.apply_explored_move(opp_board_col_major_move);
        tree.search_until(turn_start + TURN_TIME);
        best_move = tree.best_explored_move();
        tree.apply_explored_move(best_move);

        let (row, col) = util::board_col_major_move_to_2d(best_move);
        println!("{row} {col}");
    }
}

fn run_v2() {
    let input_rx = spawn_stdin_channel();

    let first_input = input_rx.recv().expect("failed to get first input");
    let (opp_row, opp_col) = first_input
        .trim_end()
        .split_once(' ')
        .expect("opponent input should have a space");
    let (opp_row, opp_col) = (
        opp_row.parse::<i32>().expect("opp_row is not i32"),
        opp_col.parse::<i32>().expect("opp_col is not i32"),
    );

    read_and_ignore_available(&input_rx);
    let initial_start_time = Instant::now();
    let inital_end_time = initial_start_time + FIRST_TURN_TIME;
    if opp_row == -1 {
        let mut tree: TreePlayer1 = TreePlayer1::new();
        tree.search_until(inital_end_time);
        run_v2_on_initialized_tree::<PLAYER1_U8>(tree, input_rx);
    } else {
        let board_idx = util::to_board_col_major_move(opp_row as u8, opp_col as u8);
        let mut tree: TreePlayer2 = TreePlayer2::new(board_idx);
        tree.search_until(inital_end_time);
        run_v2_on_initialized_tree::<PLAYER2_U8>(tree, input_rx);
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
