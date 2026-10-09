module Main where

import GHC.Clock (getMonotonicTimeNSec)
import System.Exit (exitWith, ExitCode(..))
import Data.Int (Int64)

square :: Int64 -> Int64
square x = x * x

main :: IO ()
main = do
    t0 <- getMonotonicTimeNSec
    let total = sum (map square [1..1000000 :: Int64])
    total `seq` return ()
    t1 <- getMonotonicTimeNSec
    let ns = t1 - t0
    putStrLn $ "COMPUTE_NS: " ++ show ns
    let code = fromIntegral (total `mod` 256)
    exitWith (ExitFailure code)
