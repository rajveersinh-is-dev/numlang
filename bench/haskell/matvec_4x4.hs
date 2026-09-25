module Main where

import System.Exit (exitWith, ExitCode(..))

dot4 :: (Int, Int, Int, Int) -> (Int, Int, Int, Int) -> Int
dot4 (m0, m1, m2, m3) (v0, v1, v2, v3) = m0 * v0 + m1 * v1 + m2 * v2 + m3 * v3

matvecStep :: (Int, Int, Int, Int) -> Int
matvecStep v =
    let r0 = dot4 (2, 1, -1, 0) v
        r1 = dot4 (-1, 3, 0, 2) v
        r2 = dot4 (0, -2, 4, 1) v
        r3 = dot4 (1, 0, 1, 3) v
    in (r0 + r1 + r2 + r3) `mod` 1000

main :: IO ()
main = do
    let total = sum [ matvecStep (i `mod` 10, (i + 1) `mod` 10, (i + 2) `mod` 10, (i + 3) `mod` 10) | i <- [0..999] ]
    print total
    let code = total `mod` 256
    exitWith (ExitFailure code)
