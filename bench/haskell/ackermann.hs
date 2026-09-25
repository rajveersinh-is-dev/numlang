module Main where

import System.Exit (exitWith, ExitCode(..))

ack :: Int -> Int -> Int
ack 0 n = n + 1
ack m 0 = ack (m - 1) 1
ack m n = ack (m - 1) (ack m (n - 1))

main :: IO ()
main = do
    let total = sum [ ack 3 4 | _ <- [1..50] ]
    print total
    let code = total `mod` 256
    exitWith (ExitFailure code)
