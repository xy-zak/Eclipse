@echo off
rem Grant Authenticated Users start/stop/query/change-config on SunshineService
rem so the Eclipse GUI can toggle start-on-boot without elevation.
rem Replaces the DACL; keep SYSTEM and Administrators full control.
sc sdset SunshineService "D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWLOCRRC;;;IU)(A;;CCLCSWLOCRRC;;;SU)(A;;CCDCLCSWRPWPDTLOCRRC;;;AU)"
