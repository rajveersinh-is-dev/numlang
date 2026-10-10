module Main where

import GHC.Clock (getMonotonicTimeNSec)
import System.Exit (exitWith, ExitCode(..))

nrev :: [Int] -> [Int]
nrev [] = []
nrev (x:xs) = nrev xs ++ [x]

main :: IO ()
main = do
    t0 <- getMonotonicTimeNSec
    let computeSum = sum [ sum (nrev (nrev [15, 14 .. 1])) | _ <- [1..100 :: Int] ]
    computeSum `seq` return ()
    t1 <- getMonotonicTimeNSec
    let ns = t1 - t0
    putStrLn (show computeSum)
    putStrLn $ "COMPUTE_NS: " ++ show ns
    let code = computeSum `mod` 256
    exitWith (ExitFailure code)
