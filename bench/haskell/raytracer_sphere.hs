module Main where

import System.Exit (exitWith, ExitCode(..))

dot3 :: (Int, Int, Int) -> (Int, Int, Int) -> Int
dot3 (x1, y1, z1) (x2, y2, z2) = x1 * x2 + y1 * y2 + z1 * z2

intersectSphere :: (Int, Int, Int) -> (Int, Int, Int) -> (Int, Int, Int) -> Int -> Int
intersectSphere (ox, oy, oz) (dx, dy, dz) (cx, cy, cz) r =
    let oc = (ox - cx, oy - cy, oz - cz)
        a = dot3 (dx, dy, dz) (dx, dy, dz)
        b = 2 * dot3 oc (dx, dy, dz)
        c = dot3 oc oc - r * r
        disc = b * b - 4 * a * c
    in if disc >= 0 then 1 else 0

main :: IO ()
main = do
    let hits = sum [ intersectSphere (0, 0, 0) (x - 25, y - 25, 50) (0, 0, 50) 10 | x <- [0..49], y <- [0..49] ]
    print hits
    let code = hits `mod` 256
    exitWith (ExitFailure code)
