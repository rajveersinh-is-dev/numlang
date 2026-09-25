module Main where

import System.Exit (exitWith, ExitCode(..))

nrev :: [Int] -> [Int]
nrev [] = []
nrev (x:xs) = nrev xs ++ [x]

main :: IO ()
main = do
    let computeSum = sum [ sum (nrev [15, 14 .. 1]) | _ <- [1..100] ]
    print computeSum
    let code = computeSum `mod` 256
    exitWith (ExitFailure code)
