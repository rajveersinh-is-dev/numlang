module Main where

import GHC.Clock (getMonotonicTimeNSec)
import System.Exit (exitWith, ExitCode(..))
import Data.Int (Int64)

streamPipeline :: Int64 -> Int64
streamPipeline n = sum [ x * x | x <- [0..n-1], even x ]

main :: IO ()
main = do
    t0 <- getMonotonicTimeNSec
    let grandTotal = sum [ streamPipeline 50 | _ <- [1..100000 :: Int] ]
    grandTotal `seq` return ()
    t1 <- getMonotonicTimeNSec
    let ns = t1 - t0
    putStrLn $ "COMPUTE_NS: " ++ show ns
    let code = fromIntegral (grandTotal `mod` 256)
    exitWith (ExitFailure code)
