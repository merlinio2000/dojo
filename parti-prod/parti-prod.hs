partiProd :: Int -> ([[Int]], Int)
partiProd 1 = ([[1]], 1)
partiProd n = case divMod n 3 of
  (d, 0) -> ([replicate d 3], 3 ^ d)
  (d, 1) ->
    ( [4 : replicate (d - 1) 3, replicate (d - 1) 3 ++ [2, 2]],
      3 ^ (d - 1) * 4
    )
  (d, 2) ->
    ( [replicate d 3 ++ [2]],
      3 ^ d * 2
    )
