module Main where

import System.Exit (exitWith, ExitCode(..))

isPrime :: Int -> Bool
isPrime n
    | n <= 1 = False
    | otherwise = null [ d | d <- [2 .. floor (sqrt (fromIntegral n :: Double))], n `mod` d == 0 ]

countPrimes :: Int -> Int
countPrimes limit = sum [ p | p <- [2..limit], isPrime p ]

main :: IO ()
main = do
    let total = sum [ countPrimes 200 | _ <- [1..100] ]
    print total
    let code = total `mod` 256
    exitWith (ExitFailure code)
