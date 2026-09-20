@echo off
python "%~dp0audit_linker.py" %*
exit /b %errorlevel%
