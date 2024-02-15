mod bigbrain;
mod treestuff;

fn main() {
    println!("f(8)  -> {:?}", bigbrain::parti_prod(8));
    println!("f(10) -> {:?}", bigbrain::parti_prod(10));
}

#[cfg(test)]
mod test {
    use super::bigbrain;
    use super::treestuff;

    #[test]
    fn deliver_same_result() {
        for i in 1..=50 {
            let (partis, prod) = bigbrain::parti_prod(i);
            println!("n={i:3} prod={prod:8?} {partis:?}");
            assert_eq!((partis, prod), treestuff::parti_prod(i));
        }
    }
}
