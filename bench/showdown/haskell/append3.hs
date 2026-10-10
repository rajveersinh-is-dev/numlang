module Main where

import GHC.Clock (getMonotonicTimeNSec)
import System.Exit (exitWith, ExitCode(..))

append3 :: [Int] -> [Int] -> [Int] -> [Int]
append3 xs ys zs = (xs ++ ys) ++ zs

main :: IO ()
main = do
    t0 <- getMonotonicTimeNSec
    let computeSum = sum [ sum (append3 [1..10] [11..20] [21..30]) | _ <- [1..100 :: Int] ]
    computeSum `seq` return ()
    t1 <- getMonotonicTimeNSec
    let ns = t1 - t0
    putStrLn (show computeSum)
    putStrLn $ "COMPUTE_NS: " ++ show ns
    let code = computeSum `mod` 256
    exitWith (ExitFailure code)
