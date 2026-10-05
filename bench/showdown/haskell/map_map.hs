module Main where

import GHC.Clock (getMonotonicTimeNSec)
import System.Exit (exitWith, ExitCode(..))
import Data.Int (Int64)

inc :: Int64 -> Int64
inc x = x + 1

double :: Int64 -> Int64
double x = x * 2

runPass :: Int64
runPass = sum (map double (map inc [0..19 :: Int64]))

main :: IO ()
main = do
    t0 <- getMonotonicTimeNSec
    let computeSum = sum [ runPass | _ <- [1..100000 :: Int] ]
    t1 <- getMonotonicTimeNSec
    let ns = t1 - t0
    putStrLn $ "COMPUTE_NS: " ++ show ns
    let code = fromIntegral (computeSum `mod` 256)
    exitWith (ExitFailure code)
