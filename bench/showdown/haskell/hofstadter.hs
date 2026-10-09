module Main where

import GHC.Clock (getMonotonicTimeNSec)
import System.Exit (exitWith, ExitCode(..))
import Data.Array (Array, array, (!))

hofstadterM :: Int -> Int
hofstadterM limit = m ! limit
  where
    f = array (0, limit) [(i, calcF i) | i <- [0..limit]]
    m = array (0, limit) [(i, calcM i) | i <- [0..limit]]
    calcF 0 = 1
    calcF i = i - m ! (f ! (i - 1))
    calcM 0 = 0
    calcM i = i - f ! (m ! (i - 1))

main :: IO ()
main = do
    t0 <- getMonotonicTimeNSec
    let res = hofstadterM 100
    res `seq` return ()
    t1 <- getMonotonicTimeNSec
    let ns = t1 - t0
    putStrLn $ "COMPUTE_NS: " ++ show ns
    let code = fromIntegral (res `mod` 256)
    exitWith (ExitFailure code)
