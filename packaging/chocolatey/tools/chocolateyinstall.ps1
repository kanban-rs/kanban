$ErrorActionPreference = 'Stop'
$toolsDir = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"
$version  = '$version$'
$url64    = "https://github.com/kanban-rs/kanban/releases/download/v$version/kanban-v$version-x86_64-pc-windows-msvc.zip"

$packageArgs = @{
  packageName    = 'kanban'
  unzipLocation  = $toolsDir
  url64bit       = $url64
  checksum64     = '$checksum64$'
  checksumType64 = 'sha256'
}

Install-ChocolateyZipPackage @packageArgs
