use ultimate_tic_tac_toe::{
    board::{Board, move_finder::BoardMoveFinder},
    types::{Index, Player},
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

fn main() {
    assert!(std::is_x86_feature_detected!("bmi1"));
    assert!(std::is_x86_feature_detected!("bmi2"));
    assert!(std::is_x86_feature_detected!("avx"));
    assert!(std::is_x86_feature_detected!("avx2"));
}
