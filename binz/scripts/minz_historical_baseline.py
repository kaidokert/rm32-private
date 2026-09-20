"""Freeze and summarize historical FALCON evidence; NOT frozen-AM32 parity.

Uses the parser from our existing frozen source archive, never a live import.
Current is nominal INA180/DC-link conversion, not DRV phase-shunt conversion.
"""
import argparse
import hashlib
import json
from pathlib import Path
import statistics
import zipfile


def summarize(frames, raw_to_ma):
    if len(frames)<100:
        raise ValueError('insufficient historical windows')
    lengths=[f['len_us'] for f in frames]
    gaps=sum((a['seq']+1)%256!=b['seq'] for a,b in zip(frames,frames[1:]))
    order_bad=sum((a['sector']+1)%6!=b['sector'] for a,b in zip(frames,frames[1:]))
    missing=[f['qzc_off_us']==65535 for f in frames]
    longest=run=0
    for i,miss in enumerate(missing):
        if i and (frames[i-1]['seq']+1)%256!=frames[i]['seq']:
            run=0
        run=run+1 if miss else 0
        longest=max(longest,run)
    duration=sum(lengths)
    return dict(windows=len(frames),sequence_discontinuities=gaps,sector_discontinuities=order_bad,
                observed_window_seconds=duration/1e6,
                frequency_from_mean_window_ehz=1e6/(6*statistics.mean(lengths)),
                window_sigma_us=statistics.pstdev(lengths),
                qzc_percent=100*(1-sum(missing)/len(frames)),missing_qzc_windows=sum(missing),
                longest_observed_missing_run=longest,
                nominal_window_mean_current_ma=raw_to_ma(statistics.mean(f['i_avg'] for f in frames)),
                nominal_time_weighted_current_ma=raw_to_ma(sum(f['i_avg']*f['len_us'] for f in frames)/duration),
                comparable_to_binz_cycle_sigma=False,crc_available=False,
                frozen_am32_firmware_provenance_verified=False,parity_proven=False)


def verify_archive(archive, reference):
    """Recompute retained metrics without reading the disconnected/live bench."""
    expected='2799b284bca75ff97e7d5a8cb57eae4ba0ce32f581e80203f6bc7168e9606c44'
    if hashlib.sha256(reference.read_bytes()).hexdigest()!=expected:
        raise ValueError('unexpected frozen reference hash')
    with zipfile.ZipFile(reference) as z:
        trusted_parser=z.read('minz/scripts/magpie.py')
    with zipfile.ZipFile(archive) as z:
        manifest=json.loads(z.read('manifest.json'))
        if set(manifest['metrics'])!={'lockmap_a9.bin','lockmap_a10.bin','lockmap20_a9.bin'}:
            raise ValueError('missing or unexpected historical captures')
        if manifest['reference_archive_sha256']!=expected:
            raise ValueError('historical reference mismatch')
        names=z.namelist()
        if len(names)!=len(set(names)) or set(names)!=set(manifest['files'])|{'manifest.json'}:
            raise ValueError('historical archive membership mismatch')
        for name,digest in manifest['files'].items():
            if hashlib.sha256(z.read(name)).hexdigest()!=digest:
                raise ValueError('historical payload hash mismatch: '+name)
        if z.read('parser/magpie.py')!=trusted_parser:
            raise ValueError('historical parser differs from trusted frozen source')
        namespace={'__name__':'frozen_magpie'}
        exec(compile(trusted_parser,'frozen/minz/scripts/magpie.py','exec'),namespace)
        reports={name:summarize(namespace['parse_frames'](z.read('captures/'+name)),
                                namespace['raw_to_ma']) for name in manifest['metrics']}
        if reports!=manifest['metrics']:
            raise ValueError('historical metrics do not reproduce')
    return dict(archive_sha256=hashlib.sha256(archive.read_bytes()).hexdigest(),
                metrics=reports,metrics_reproduced=True,
                capture_firmware_provenance_verified=False,parity_proven=False)


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--minz',type=Path,default=Path('../minz'))
    ap.add_argument('--reference',type=Path,default=Path('captures/reference/minz_20260912.zip'))
    mode=ap.add_mutually_exclusive_group(required=True)
    mode.add_argument('--out',type=Path)
    mode.add_argument('--verify-archive',type=Path)
    args=ap.parse_args()
    if args.verify_archive:
        print(json.dumps(verify_archive(args.verify_archive,args.reference),indent=2))
        return
    if args.out.exists():
        ap.error('refusing to overwrite evidence archive')
    reference=args.reference.read_bytes()
    expected='2799b284bca75ff97e7d5a8cb57eae4ba0ce32f581e80203f6bc7168e9606c44'
    if hashlib.sha256(reference).hexdigest()!=expected:
        raise ValueError('unexpected frozen reference hash')
    with zipfile.ZipFile(args.reference) as z:
        parser=z.read('minz/scripts/magpie.py')
    namespace={'__name__':'frozen_magpie'}
    exec(compile(parser,'frozen/minz/scripts/magpie.py','exec'),namespace)
    payloads={'parser/magpie.py':parser}
    reports={}
    for name in ['lockmap_a9.bin','lockmap_a10.bin','lockmap20_a9.bin']:
        data=(args.minz/'captures'/name).read_bytes()
        payloads['captures/'+name]=data
        reports[name]=summarize(namespace['parse_frames'](data),namespace['raw_to_ma'])
    for name in ['lockmap_session.log','lockmap20_session.log']:
        payloads['captures/'+name]=(args.minz/'captures'/name).read_bytes()
    payloads['FALCON_HARDENING.md']=(args.minz/'FALCON_HARDENING.md').read_bytes()
    manifest=dict(reference_archive_sha256=expected,
                  limitation='Historical FALCON, different board/supply/controller; capture firmware hash unverified. Not AM32 parity.',
                  files={n:hashlib.sha256(b).hexdigest() for n,b in payloads.items()},metrics=reports)
    with zipfile.ZipFile(args.out,'x',compression=zipfile.ZIP_DEFLATED) as z:
        for name,data in payloads.items():
            z.writestr(name,data)
        z.writestr('manifest.json',json.dumps(manifest,indent=2)+'\n')
    print(json.dumps(manifest,indent=2))


if __name__=='__main__':
    main()
