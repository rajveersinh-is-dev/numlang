module Main where

import System.Exit (exitWith, ExitCode(..))

streamPipeline :: Int -> Int
streamPipeline n = sum [ x * x | x <- [0 .. n - 1], even x ]

main :: IO ()
main = do
    let grandTotal = sum [ streamPipeline 50 `mod` 10000 | _ <- [1..1000] ]
    print grandTotal
    let code = grandTotal `mod` 256
    exitWith (ExitFailure code)
