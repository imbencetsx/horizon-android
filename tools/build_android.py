#!/usr/bin/env python3
"""Build a development APK from the shared Rust game; no Java/Gradle required."""
import argparse
import os
from pathlib import Path
import platform
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def run(*args, env=None):
    subprocess.run([str(a) for a in args], cwd=ROOT, env=env, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sdk', type=Path, default=os.environ.get('ANDROID_HOME') or os.environ.get('ANDROID_SDK_ROOT'))
    parser.add_argument('--ndk', type=Path, default=os.environ.get('ANDROID_NDK_HOME'))
    parser.add_argument('--profile', choices=['dev', 'fast', 'release'], default='fast')
    parser.add_argument('--no-build', action='store_true')
    parser.add_argument('--resolution-scale', type=int, choices=range(50, 101), default=50, metavar='50..100', help='first-run world resolution percentage; HUD and controls remain native')
    parser.add_argument('--section-batching', choices=['auto', 'off'], default='auto', help='Android GLES ordered section batching; off provides a profiling comparison')
    parser.add_argument('--smoke-world', action='store_true', help='create a fresh server-thread world for device QA; requires --agent-sandbox')
    parser.add_argument('--graphics-api', choices=['auto', 'vulkan', 'opengl'], default='auto', help='first-run graphics API; opengl works around emulator Vulkan driver faults')
    parser.add_argument('--agent-sandbox', action='store_true', help='fresh app-private data for each test launch')
    args = parser.parse_args()
    if args.smoke_world and not args.agent_sandbox:
        parser.error("--smoke-world requires --agent-sandbox to protect player data")
    sdk = args.sdk or Path.home() / ('Library/Android/sdk' if platform.system() == 'Darwin' else 'Android/Sdk')
    if not sdk.is_dir():
        parser.error('Android SDK not found; set ANDROID_HOME or --sdk')
    ndks = sorted((sdk / 'ndk').glob('*'), key=lambda p: tuple(int(n) for n in p.name.split('.')))
    ndk = args.ndk or (ndks[-1] if ndks else None)
    if ndk is None:
        parser.error('Install an Android NDK and set ANDROID_NDK_HOME or --ndk')
    prebuilts = list((ndk / 'toolchains/llvm/prebuilt').glob('*'))
    if not prebuilts:
        parser.error(f'No NDK toolchain in {ndk}')
    bin_dir = prebuilts[0] / 'bin'
    tools = sdk / 'build-tools/36.0.0'
    android_jar = sdk / 'platforms/android-36/android.jar'
    for path in [tools / 'aapt2', tools / 'apksigner', tools / 'zipalign', android_jar]:
        if not path.is_file():
            parser.error(f'Missing SDK tool: {path}; install platforms;android-36 and build-tools;36.0.0')
    env = os.environ.copy()
    env.update({
        'CC_aarch64_linux_android': str(bin_dir / 'aarch64-linux-android28-clang'),
        'AR_aarch64_linux_android': str(bin_dir / 'llvm-ar'),
        'CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER': str(bin_dir / 'aarch64-linux-android28-clang'),
        'CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS': '-C link-arg=-Wl,-z,max-page-size=16384',
    })
    env['HORIZON_ANDROID_SECTION_BATCHING'] = '0' if args.section_batching == 'off' else '1'
    env['HORIZON_ANDROID_RESOLUTION_SCALE'] = str(args.resolution_scale)
    env['HORIZON_ANDROID_SMOKE_WORLD'] = '1' if args.smoke_world else '0'
    env['HORIZON_ANDROID_GRAPHICS_API'] = args.graphics_api
    env['HORIZON_ANDROID_AGENT_SANDBOX'] = '1' if args.agent_sandbox else '0'
    if not args.no_build:
        run('cargo', 'build', '-p', 'horizon-client', '--lib', '--no-default-features',
            '--target', 'aarch64-linux-android', '--profile', args.profile, '-j', '2', env=env)
    target = Path(env.get('CARGO_TARGET_DIR', ROOT / 'target'))
    lib = target / 'aarch64-linux-android' / ('debug' if args.profile == 'dev' else args.profile) / 'libhorizon_client.so'
    if not lib.is_file():
        parser.error(f'Native library missing: {lib}')
    out = target / 'android'
    out.mkdir(parents=True, exist_ok=True)
    unsigned, aligned, apk = [out / f'horizon-{name}.apk' for name in ['unsigned', 'aligned', 'debug']]
    run(tools / 'aapt2', 'link', '-I', android_jar, '--manifest', ROOT / 'android/AndroidManifest.xml', '-o', unsigned)
    with zipfile.ZipFile(unsigned, 'a', compression=zipfile.ZIP_STORED) as archive:
        archive.write(lib, 'lib/arm64-v8a/libhorizon_client.so')
    run(tools / 'zipalign', '-f', '-P', '16', '4', unsigned, aligned)
    key = out / 'development.keystore'
    if not key.exists():
        run('keytool', '-genkeypair', '-keystore', key, '-storepass', 'android', '-keypass', 'android',
            '-alias', 'androiddebugkey', '-dname', 'CN=Horizon Development', '-keyalg', 'RSA', '-validity', '10000')
    run(tools / 'apksigner', 'sign', '--ks', key, '--ks-pass', 'pass:android', '--out', apk, aligned)
    run(tools / 'apksigner', 'verify', '--verbose', apk)
    print(f'APK: {apk}')


if __name__ == '__main__':
    main()
