module Main where

import GHC.Clock (getMonotonicTimeNSec)
import System.Exit (exitWith, ExitCode(..))
import System.Environment (getArgs)
import Data.Int (Int64)

triSum :: Int64 -> Int64
triSum n = loop 0 1
  where
    loop acc i
      | i <= n    = loop (acc + i) (i + 1)
      | otherwise = acc

main :: IO ()
main = do
    args <- getArgs
    let n = if null args then 50000000 else read (head args) :: Int64
    t0 <- getMonotonicTimeNSec
    let fullRes = triSum n
    fullRes `seq` return ()
    t1 <- getMonotonicTimeNSec
    let ns = t1 - t0
    putStrLn (show fullRes)
    putStrLn $ "COMPUTE_NS: " ++ show ns
    let code = fromIntegral ((fullRes `mod` 256 + 256) `mod` 256)
    exitWith (ExitFailure code)
