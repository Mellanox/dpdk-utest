# dpdk-utest

## configuration file format:
```yaml
dut:  
  # DUT hostname or IP address  
  host:  
  # DUT username  
  username:  
  # DUT user password  
  password:  
  # DUT path to test program  
  path:  
  
tg:  
  # TG hostname or IP address  
  host:  
  # TG username  
  username:  
  # TG password  
  password:  

vm:
    # VM hostname or IP address  
    host:
    # VM username  
    username:
    # VM password  
    password:  
```

## test file format
``` yaml
prog: 'dpdk-testpmd -a PORT_0_params -a PORT_1_params -- -i'

flow:
  -
    phases: [ *phase1 ]
    repeat: 1
  -
    phases: [ *phase2, *phase3 ]
    repeat: 3
    
p: &phase
  # phase name - mandatory
  name:
  
  # PMD commands - optional
  pmd:
    - 
      # testpmd commands
      command:
      # string to match after command execution or None
      result:

  # TG command - optional     
  tg:

  # VM command - optional
  vm:

  # phase results to match - optional
  results:
    pmd:
    tg:
    vm:
      
  # phase repeat counter - optional
  repeat:
  
setup:
  hca: # any  or  mt<ID> 
  fw: # any or fw version
  hws: # True|False
  pf: [<nic|fdb>, <nic|fdb>]
  vf: [<VF num>, <VF num>]
  sf: [<SF num>, <SF num>]  
```      
  
### Phase execution order:
Execution order of `pmd` and `tg` keys depends on the keys position.
```
phase1:
    name:
    pmd:
    tg:

phase2:
    name:
    tg:
    pmd:
```

In `phase1` `tg` follows `pmd` while in `phase2` `pmd` follows `tg`.    
