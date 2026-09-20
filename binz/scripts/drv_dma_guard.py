"""Explicit opt-in for IRQ safety publication; old captures omit guard_irq."""
import re


def verify(text, required=False):
    lines = re.findall(r'^DMAFEEDBACK [^\r\n]*', text, re.M)
    if len(lines) != 1:
        raise ValueError('expected one DMAFEEDBACK record')
    fields = re.findall(r'\bguard_irq=(\d+)\b', lines[0])
    if fields not in ([], ['0'], ['1']):
        raise ValueError('invalid DMA guard publication mode')
    if (fields == ['1']) != required:
        raise ValueError('DMA guard publication needs matching --dma-guard')
    return required
