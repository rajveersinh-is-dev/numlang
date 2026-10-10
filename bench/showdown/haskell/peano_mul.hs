module Main where

import GHC.Clock (getMonotonicTimeNSec)
import System.Exit (exitWith, ExitCode(..))

data Peano = Zero | Succ Peano

addPeano :: Peano -> Peano -> Peano
addPeano Zero y = y
addPeano (Succ p) y = Succ (addPeano p y)

mulPeano :: Peano -> Peano -> Peano
mulPeano Zero _ = Zero
mulPeano (Succ p) y = addPeano y (mulPeano p y)

toInt :: Peano -> Int
toInt Zero = 0
toInt (Succ p) = 1 + toInt p

fromInt :: Int -> Peano
fromInt n
    | n <= 0    = Zero
    | otherwise = Succ (fromInt (n - 1))

main :: IO ()
main = do
    t0 <- getMonotonicTimeNSec
    let computeSum = sum [ toInt (mulPeano (fromInt 3) (fromInt 4)) | _ <- [1..100 :: Int] ]
    computeSum `seq` return ()
    t1 <- getMonotonicTimeNSec
    let ns = t1 - t0
    putStrLn (show computeSum)
    putStrLn $ "COMPUTE_NS: " ++ show ns
    let code = computeSum `mod` 256
    exitWith (ExitFailure code)
