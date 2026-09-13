library ieee;
use ieee.std_logic_1164.all;
entity counter is port(clk, reset, data: in std_logic; q: out std_logic); end entity;
architecture rtl of counter is
  function twice(value: integer) return integer is
    variable result: integer;
  begin result := value * 2; return result; end function;
  function doubled(input: integer) return integer is
    variable output: integer;
  begin output := input * 2; return output; end function;
  procedure report_value(value: integer) is
  begin report integer'image(value); end procedure;
begin
  first: process(clk)
  begin if rising_edge(clk) then if reset = '1' then q <= '0'; else q <= data; end if; end if; end process;
  second: process(clk)
  begin if rising_edge(clk) then if reset = '1' then q <= '0'; else q <= data; end if; end if; end process;
end architecture;
