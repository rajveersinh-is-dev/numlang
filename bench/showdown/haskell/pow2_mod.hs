module Main where

import GHC.Clock (getMonotonicTimeNSec)
import System.Exit (exitWith, ExitCode(..))
import Data.Int (Int64)

pow2 :: Int64 -> Int64
pow2 n = loop 1 0
  where
    loop acc i
      | i < n     = loop (acc * 2) (i + 1)
      | otherwise = acc `mod` 256

main :: IO ()
main = do
    t0 <- getMonotonicTimeNSec
    let r = pow2 100
    r `seq` return ()
    t1 <- getMonotonicTimeNSec
    let ns = t1 - t0
    putStrLn (show r)
    putStrLn $ "COMPUTE_NS: " ++ show ns
    let code = fromIntegral ((r `mod` 256 + 256) `mod` 256)
    exitWith (ExitFailure code)
