@echo off
rem Grant Authenticated Users start/stop/query on SunshineService.
rem Replaces the DACL; keep SYSTEM and Administrators full control.
sc sdset SunshineService "D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWLOCRRC;;;IU)(A;;CCLCSWLOCRRC;;;SU)(A;;RPWPCR;;;AU)"
