module Main where

import System.Exit (exitWith, ExitCode(..))

append3 :: [Int] -> [Int] -> [Int] -> [Int]
append3 xs ys zs = (xs ++ ys) ++ zs

main :: IO ()
main = do
    let computeSum = sum [ sum (append3 [1..10] [11..20] [21..30]) | _ <- [1..100] ]
    print computeSum
    let code = computeSum `mod` 256
    exitWith (ExitFailure code)
