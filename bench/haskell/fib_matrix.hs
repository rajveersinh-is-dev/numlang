module Main where

import System.Exit (exitWith, ExitCode(..))

fibCoupled :: Int -> Int
fibCoupled n = loop 0 1 0
  where
    loop a _ i | i == n = a
    loop a b i = loop b ((a + b) `mod` 1000000007) (i + 1)

main :: IO ()
main = do
    let total = foldl (\acc _ -> (acc + fibCoupled 40) `mod` 1000000007) 0 [1..1000]
    print total
    let code = total `mod` 256
    exitWith (ExitFailure code)
