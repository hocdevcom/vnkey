$ErrorActionPreference = 'Stop'
$pythonCommand = Get-Command python -ErrorAction Stop
& $pythonCommand.Source (Join-Path $PSScriptRoot 'packaging\build.py') @args
exit $LASTEXITCODE
