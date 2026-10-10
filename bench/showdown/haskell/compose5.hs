module Main where

import GHC.Clock (getMonotonicTimeNSec)
import System.Exit (exitWith, ExitCode(..))
import Data.Int (Int64)

add1, mul2, add3, sub5, add10 :: Int64 -> Int64
add1 x = x + 1
mul2 x = x * 2
add3 x = x + 3
sub5 x = x - 5
add10 x = x + 10

runChain :: Int64 -> Int64
runChain = add1 . mul2 . add3 . sub5 . add10

main :: IO ()
main = do
    t0 <- getMonotonicTimeNSec
    let computeSum = sum [ runChain i | i <- [0..999 :: Int64] ]
    computeSum `seq` return ()
    t1 <- getMonotonicTimeNSec
    let ns = t1 - t0
    putStrLn (show computeSum)
    putStrLn $ "COMPUTE_NS: " ++ show ns
    let code = fromIntegral (computeSum `mod` 256)
    exitWith (ExitFailure code)
