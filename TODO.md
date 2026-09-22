working on slayer legend relic inheritance simulator

rules are:

one "inheritance attempt" aka 1 simulation
takes 1 argument: inheritor level

inheritor level is used to calculate several things:
max spirit power = defaults to 7
memory slots = defaults to 5, but can go up to 6,7 or more
baseline chance of success: 80% 65% 50% 35% 20%
can also be modified, seperate modifiers for different spirit power

we need to full 2 bars
glory and despair
each bar has max spirit power number of slots (5,6,7...)

we also start with "mental strength" equal to the slots

and spirit power is set to max spirit power

# inheritor level bonuses
level = effect
1 = 5 memory slot and 8 max spirit power
2 = +2% glory rate
3 = -2% despair rate
4 = 6 memory slots
5 = 9 max spirit power
6 = +2% glory rate
7 = -2% despair rate
8 = 7 memory slots

9-20 repeat +2 glory, -2 despair, +1 slot, +1 spirit

# the process
at each step, we must choose 1 of 3 options:
glory, despair, mental training
glory and despair consume 1 spirit power, 
then the first empty slot becomes either success or fail, according to their chance
you cannot perform glory or despair if you have 0 spirit power, or if all slots are filled

mental training can be performed as long as mental strength > 0
consumes 1 mental strength
if success, gain +2 spirit power (but it cant overcap over the max)

after any of these 3 steps, the next baseline chance is lowered by 1 tier if you succeed, and raised by 1 tier if you fail

the attempt is over upon filling all slots for despair and glory, but results in a failure if we run out of both mental strength and spirit power
because in that case we couldnt pick any option

a failure can be treated as an automatic: all glory slots fail, all despair slots success aka the worst possible scenario

# the goal
optimize for greatest number of successes on glory
optimize for most number of fails on despair
absolutely avoid a full on failure: when there is no mental strength, spirit power left, but we havent filled all slots for despair and glory
in this case we can't pick any of the 3 options, but we didnt finish the inheritance attempt

you need to run a probability weighted simulation in order to achieve this
i want an output on the expected average glory and despair under optimal choicemaking
i also want a report on the general idea of the strategy, at inheritance level 1 for now

# report

ok create constructable optimization strategies with the following parameters
strategy(target_glory, target_despair)
our current strategy would be strategy(999, 0) as it wants max glory and min despair
i want to test another strategy (4, 2) which means it wants 4 glory, but any more than that is worthless
it also wants 2 or less despair, any less than 2 is worthless

give me a table with all of the current 8 levels x the 2 strategies showing expected glory, despair, and failure rates, also show the rate of hitting each "target" for the strategy

for 999,0 this means the percentage of attempts that reached the maximum glory (not 999 but whatever the max is for the level, and 0 despair)

for 4,2, the percentage of attempts with >=4 glory, <=2 despair