/// for: https://www.codewars.com/kata/5716a4c2794d305f4900156b
/// based on my findings from ../the-4-3-and-the-2s.xopp
///
/// Axioms:
/// 1. [4, _] and [_, 2, 2] is equivalent
///
/// 2. Never choose 1 as a factor since it doesn't increase the product
///     (except n=1)
///
/// 3. For n > 5, 3 is preferrable to choose over 2
///
/// 4. More, smaller factors are preferrable over fewer larger ones
///
/// 5. Every n >= 1 can be reduced using a combination of 1-4
///
/// See the notes document for proof

pub fn parti_prod(n: u64) -> (Vec<Vec<u64>>, u64) {
    if n <= 3 {
        return (vec![vec![n]], n);
    }

    let base_result_len: usize = (n / 3).try_into().unwrap();
    match n % 3 {
        0 => {
            let partition = vec![3; base_result_len];
            (vec![partition], 3u64.pow(base_result_len as u32))
        }
        // we reduce back to a 4 and [3,1] is no bueno -> [_,2,2] & [4,_]
        1 => {
            // only one allocation base_result_len + (base_result_len + 1)
            let mut double_partition: Vec<u64> = vec![3; 2 * base_result_len + 1];

            let mut partition_with_4 = double_partition.split_off(base_result_len + 1);
            let mut partition_with_2s = double_partition;

            partition_with_4[0] = 4;

            let len_2s = partition_with_2s.len();
            partition_with_2s[len_2s - 2] = 2;
            partition_with_2s[len_2s - 1] = 2;

            (
                vec![partition_with_4, partition_with_2s],
                3u64.pow((base_result_len - 1) as u32) * 4,
            )
        }
        // we reduce back to a 5 -> [_, 3,2]
        2 => {
            let mut partition = vec![3; base_result_len + 1];

            let len = partition.len();
            partition[len - 1] = 2;

            (vec![partition], 3u64.pow(base_result_len as u32) * 2)
        }
        _ => unreachable!(),
    }
}
