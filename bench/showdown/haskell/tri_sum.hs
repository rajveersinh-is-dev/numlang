module Main where

import GHC.Clock (getMonotonicTimeNSec)
import System.Exit (exitWith, ExitCode(..))
import Data.Int (Int64)

triSum :: Int64 -> Int64
triSum n = loop 0 1
  where
    loop acc i
      | i <= n    = loop (acc + i) (i + 1)
      | otherwise = acc

main :: IO ()
main = do
    t0 <- getMonotonicTimeNSec
    let r = triSum 50000000 `mod` 256
    t1 <- getMonotonicTimeNSec
    let ns = t1 - t0
    putStrLn $ "COMPUTE_NS: " ++ show ns
    let code = fromIntegral ((r `mod` 256 + 256) `mod` 256)
    exitWith (ExitFailure code)
