module Main where

import GHC.Clock (getMonotonicTimeNSec)
import System.Exit (exitWith, ExitCode(..))
import Data.Int (Int64)

modVal :: Int64
modVal = 1000000007

matMul :: ((Int64, Int64), (Int64, Int64)) -> ((Int64, Int64), (Int64, Int64)) -> ((Int64, Int64), (Int64, Int64))
matMul ((a00, a01), (a10, a11)) ((b00, b01), (b10, b11)) =
    ( ( (a00 * b00 + a01 * b10) `mod` modVal, (a00 * b01 + a01 * b11) `mod` modVal )
    , ( (a10 * b00 + a11 * b10) `mod` modVal, (a10 * b01 + a11 * b11) `mod` modVal )
    )

fib :: Int64 -> Int64
fib n
    | n <= 0    = 0
    | otherwise = let ((_, res), _) = powMat ((1, 0), (0, 1)) ((1, 1), (1, 0)) n in res
  where
    powMat acc base p
        | p <= 0         = acc
        | p `mod` 2 == 1 = powMat (matMul acc base) (matMul base base) (p `div` 2)
        | otherwise      = powMat acc (matMul base base) (p `div` 2)

main :: IO ()
main = do
    t0 <- getMonotonicTimeNSec
    let f40 = fib 40
    let computeSum = (f40 * 1000) `mod` modVal
    computeSum `seq` return ()
    t1 <- getMonotonicTimeNSec
    let ns = t1 - t0
    putStrLn (show computeSum)
    putStrLn $ "COMPUTE_NS: " ++ show ns
    let code = fromIntegral (computeSum `mod` 256)
    exitWith (ExitFailure code)
