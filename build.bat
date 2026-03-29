@echo off
call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvarsall.bat" x64
echo VCVARS_DONE
echo LIB=%LIB%
cd /d E:\soft\tech-trands
cargo check
echo EXIT_CODE=%ERRORLEVEL%
