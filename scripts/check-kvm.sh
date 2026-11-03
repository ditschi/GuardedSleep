#!/usr/bin/env bash
# Diagnose KVM availability for the Docker Android emulator (hybrid E2E).
set -euo pipefail

echo "=== KVM / virtualization check ==="
echo ""

fail=0

if grep -Eq 'vmx|svm' /proc/cpuinfo 2>/dev/null; then
	echo "OK   CPU virtualization flag in /proc/cpuinfo (vmx/svm)"
else
	echo "FAIL CPU virtualization extension not visible (BIOS/VM nesting?)"
	fail=1
fi

if [[ -e /dev/kvm ]]; then
	echo "OK   /dev/kvm exists ($(ls -l /dev/kvm))"
	if [[ -r /dev/kvm && -w /dev/kvm ]]; then
		echo "OK   /dev/kvm readable/writable for $(id -un)"
	else
		echo "WARN /dev/kvm not writable — try: sudo usermod -aG kvm $(id -un)  (re-login)"
	fi
else
	echo "FAIL /dev/kvm missing"
	echo "     Try: sudo modprobe kvm && sudo modprobe kvm_intel   # Intel"
	echo "           sudo modprobe kvm && sudo modprobe kvm_amd    # AMD"
	echo "     Enable virtualization (VT-x/AMD-V) in BIOS/UEFI."
	echo "     Inside a VM: enable nested virtualization for the guest."
	fail=1
fi

echo ""
if [[ "$fail" -eq 0 ]]; then
	echo "Result: KVM ready — hybrid docker emulator should use hardware acceleration."
else
	echo "Result: KVM not ready — browser E2E (just e2e) does not need it"
	echo "        Hybrid emulator falls back to software mode (EMULATOR_ALLOW_NO_KVM=1) or is skipped."
fi

exit "$fail"
