[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$InputExe,
    [Parameter(Mandatory = $true)][string]$IconPath
)

$ErrorActionPreference = 'Stop'
$exe = (Resolve-Path -LiteralPath $InputExe).Path
$ico = (Resolve-Path -LiteralPath $IconPath).Path
if ([IO.Path]::GetExtension($exe) -ine '.exe') { throw 'InputExe must be an .exe file.' }
if ([IO.Path]::GetExtension($ico) -ine '.ico') { throw 'IconPath must be an .ico file.' }
if ([string]::Equals($exe, $ico, [StringComparison]::OrdinalIgnoreCase)) { throw 'InputExe and IconPath must be different files.' }

if (-not ('MascotIconResourceWriter' -as [type])) {
    $source = @"
using System;
using System.Runtime.InteropServices;

public static class MascotIconResourceWriter
{
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr BeginUpdateResourceW(string fileName, bool deleteExistingResources);

    [DllImport("kernel32.dll", EntryPoint = "UpdateResourceW", CharSet = CharSet.Unicode, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool UpdateResourceW(IntPtr update, IntPtr type, IntPtr name,
        ushort language, byte[] data, uint dataSize);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool EndUpdateResourceW(IntPtr update, bool discard);

    public static IntPtr ResourceId(ushort id) { return new IntPtr(id); }
}
"@
    Add-Type -TypeDefinition $source
}

$bytes = [IO.File]::ReadAllBytes($ico)
if ($bytes.Length -lt 6 -or $bytes[0] -ne 0 -or $bytes[1] -ne 0 -or $bytes[2] -ne 1 -or $bytes[3] -ne 0) {
    throw 'IconPath is not a valid Windows ICO file.'
}
$count = [BitConverter]::ToUInt16($bytes, 4)
if ($count -eq 0 -or $count -gt 256) { throw "ICO image count is invalid: $count" }
$directoryEnd = 6 + 16 * [int]$count
if ($directoryEnd -gt $bytes.Length) { throw 'ICO directory is truncated.' }

# RT_GROUP_ICON entries mirror the ICO directory except that each image offset
# is replaced by the integer resource ID of its corresponding RT_ICON payload.
$group = New-Object byte[] (6 + 14 * [int]$count)
[Array]::Copy($bytes, 0, $group, 0, 6)
$images = @()
for ($i = 0; $i -lt $count; $i++) {
    $icoEntry = 6 + 16 * $i
    $resourceId = [uint16](0x7001 + $i)
    $size = [BitConverter]::ToUInt32($bytes, $icoEntry + 8)
    $offset = [BitConverter]::ToUInt32($bytes, $icoEntry + 12)
    if ($size -eq 0 -or $offset -lt $directoryEnd -or ([uint64]$offset + [uint64]$size) -gt $bytes.Length) {
        throw "ICO image entry $i has invalid bounds."
    }
    $payload = New-Object byte[] ([int]$size)
    [Array]::Copy($bytes, [int]$offset, $payload, 0, [int]$size)
    $images += ,@{ Id = $resourceId; Bytes = $payload }

    $groupEntry = 6 + 14 * $i
    [Array]::Copy($bytes, $icoEntry, $group, $groupEntry, 8)
    [Array]::Copy($bytes, $icoEntry + 8, $group, $groupEntry + 8, 4)
    [BitConverter]::GetBytes($resourceId).CopyTo($group, $groupEntry + 12)
}

$handle = [MascotIconResourceWriter]::BeginUpdateResourceW($exe, $false)
if ($handle -eq [IntPtr]::Zero) {
    $errorCode = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
    throw "BeginUpdateResourceW failed for '$exe' (Win32 $errorCode)."
}
$committed = $false
try {
    foreach ($image in $images) {
        $ok = [MascotIconResourceWriter]::UpdateResourceW(
            $handle,
            [MascotIconResourceWriter]::ResourceId(3),
            [MascotIconResourceWriter]::ResourceId($image.Id),
            [uint16]0,
            $image.Bytes,
            [uint32]$image.Bytes.Length)
        if (-not $ok) {
            $errorCode = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
            throw "UpdateResourceW(RT_ICON $($image.Id)) failed (Win32 $errorCode)."
        }
    }
    $ok = [MascotIconResourceWriter]::UpdateResourceW(
        $handle,
        [MascotIconResourceWriter]::ResourceId(14),
        [MascotIconResourceWriter]::ResourceId(0x7000),
        [uint16]0,
        $group,
        [uint32]$group.Length)
    if (-not $ok) {
        $errorCode = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
        throw "UpdateResourceW(RT_GROUP_ICON) failed (Win32 $errorCode)."
    }
    if (-not [MascotIconResourceWriter]::EndUpdateResourceW($handle, $false)) {
        $errorCode = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
        throw "EndUpdateResourceW failed (Win32 $errorCode)."
    }
    $committed = $true
} finally {
    if (-not $committed) {
        [void][MascotIconResourceWriter]::EndUpdateResourceW($handle, $true)
    }
}

Write-Output "Embedded $count icon images from '$ico' into '$exe'."
