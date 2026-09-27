Yes—**motor loadable-byte restoration is supported by the supplied hashes**, conditional on the captures accurately representing the loadable sections.

- C2 archived motor and E478 rebuilt motor both hash to `7EB9AA00EE5A270F0F77DCECFF047822BBD9E0DE5CD236860CABF596C6E34C9B`.
- E477 rebuilt motor differs: `41E0CD0463A0DCBE9C1143E9C6D204D3FDFEF630D9AE49EAFED9F0F6AC9AA6AE`.
- D677 archived/E477 rebuilt probe match at `12892E47ABC9C10AB588C35D8F07287F71B8337E78ABEA425C599BAB7003FD9C`; E478 probe changed to `268089D5F2CAA85234C58BA1FF5563FB7A471973BA1AE9F9082C49F9F15B2D79`.

E477 motor audit **failed**: ADC_COMP reached six functions and contained an unreviewed `offer_timed` loop. E478 **passed all four roots**; ADC_COMP reachability fell to three.

The generic-entry change supports attributing restoration to moving `ProbeWindow` instantiation into the disabled binary. Hash identity establishes captured loadable-byte identity, not whole-ELF identity or independently demonstrated runtime behavior. The audit establishes its reported static checks, not hardware acceptance.

**The changed E478 probe remains untested.** Source assertions describe intended checks; no new flashing or execution evidence establishes their success.
