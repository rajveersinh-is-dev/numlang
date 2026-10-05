module Main where

import GHC.Clock (getMonotonicTimeNSec)
import System.Exit (exitWith, ExitCode(..))

data List = Nil | Cons Int List

matchState0 :: List -> Int
matchState0 Nil = 0
matchState0 (Cons c rest)
    | c == 1    = matchState1 rest
    | otherwise = matchState0 rest

matchState1 :: List -> Int
matchState1 Nil = 0
matchState1 (Cons c rest)
    | c == 0    = matchState2 rest
    | otherwise = matchState1 rest

matchState2 :: List -> Int
matchState2 Nil = 0
matchState2 (Cons c rest)
    | c == 1    = 1 + matchState1 rest
    | otherwise = matchState0 rest

makeText :: Int -> List
makeText n
    | n <= 0    = Nil
    | otherwise = Cons ((n * 73 + 19) `mod` 2) (makeText (n - 1))

main :: IO ()
main = do
    t0 <- getMonotonicTimeNSec
    let computeSum = sum [ matchState0 (makeText 15) | _ <- [1..100 :: Int] ]
    t1 <- getMonotonicTimeNSec
    let ns = t1 - t0
    putStrLn $ "COMPUTE_NS: " ++ show ns
    let code = computeSum `mod` 256
    exitWith (ExitFailure code)
