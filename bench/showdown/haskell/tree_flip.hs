module Main where

import GHC.Clock (getMonotonicTimeNSec)
import System.Exit (exitWith, ExitCode(..))

data Tree = Leaf Int | Node Tree Tree

flipTree :: Tree -> Tree
flipTree (Leaf v) = Leaf v
flipTree (Node l r) = Node (flipTree r) (flipTree l)

sumTree :: Tree -> Int
sumTree (Leaf v) = v
sumTree (Node l r) = sumTree l + sumTree r

makeTree :: Int -> Int -> Tree
makeTree depth val
    | depth <= 0 = Leaf val
    | otherwise  = Node (makeTree (depth - 1) (val * 2)) (makeTree (depth - 1) (val * 2 + 1))

main :: IO ()
main = do
    t0 <- getMonotonicTimeNSec
    let computeSum = sum [ sumTree (flipTree (flipTree (makeTree 4 1))) | _ <- [1..100 :: Int] ]
    computeSum `seq` return ()
    t1 <- getMonotonicTimeNSec
    let ns = t1 - t0
    putStrLn (show computeSum)
    putStrLn $ "COMPUTE_NS: " ++ show ns
    let code = computeSum `mod` 256
    exitWith (ExitFailure code)
