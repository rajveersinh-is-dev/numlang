module Main where

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
    let total = sum [ sumTree (flipTree (flipTree (makeTree 4 1))) | _ <- [1..100] ]
    print total
    let code = total `mod` 256
    exitWith (ExitFailure code)
