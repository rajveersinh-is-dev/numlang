module Main where

import GHC.Clock (getMonotonicTimeNSec)
import System.Exit (exitWith, ExitCode(..))
import Data.Int (Int64)

mutX :: Int64 -> Int64 -> Int64 -> Int64
mutX 0 x _ = x
mutX n x y = mutY (n - 1) (x + y) x

mutY :: Int64 -> Int64 -> Int64 -> Int64
mutY 0 _ y = y
mutY n x y = mutX (n - 1) y (x + y)

main :: IO ()
main = do
    t0 <- getMonotonicTimeNSec
    let computeSum = sum [ mutX 6 1 2 | _ <- [1..10000 :: Int] ]
    t1 <- getMonotonicTimeNSec
    let ns = t1 - t0
    putStrLn $ "COMPUTE_NS: " ++ show ns
    let code = fromIntegral (computeSum `mod` 256)
    exitWith (ExitFailure code)
