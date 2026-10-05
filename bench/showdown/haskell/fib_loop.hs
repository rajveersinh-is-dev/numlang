module Main where

import GHC.Clock (getMonotonicTimeNSec)
import System.Exit (exitWith, ExitCode(..))
import Data.Int (Int64)

fib :: Int64 -> Int64
fib n = loop 0 1 0
  where
    loop a b i
      | i < n     = loop b (a + b) (i + 1)
      | otherwise = a

main :: IO ()
main = do
    t0 <- getMonotonicTimeNSec
    let r = fib 1000000 `mod` 256
    t1 <- getMonotonicTimeNSec
    let ns = t1 - t0
    putStrLn $ "COMPUTE_NS: " ++ show ns
    let code = fromIntegral ((r `mod` 256 + 256) `mod` 256)
    exitWith (ExitFailure code)
