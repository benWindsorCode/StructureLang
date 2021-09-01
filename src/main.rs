extern crate pest;
#[macro_use]
extern crate pest_derive;

use pest::Parser;
use pest::error::Error;
use std::fs;
use std::collections::HashMap;
use std::ops::Add;
use std::iter::Sum;

#[derive(Parser)]
#[grammar = "array_language_grammar.pest"]
struct ArrayLanguageParser;


#[derive(Debug, PartialEq, Eq, Clone)]
pub enum DyadicVerb {
    Add,
    Divide,
    Replicate,
    GreaterThan
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum MonadicVerb {
    Print,
    Generate,
    Shape
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum OperatorVerb {
    Reduce
}

#[derive(Debug, Copy, PartialEq, Clone, PartialOrd)]
pub enum Numeric {
    Int(i64),
    Float(f64)
}

impl Add for Numeric {
    type Output = Numeric;

    fn add(self, other: Numeric) -> Numeric {
        match (self, other) {
            (Numeric::Int(a), Numeric::Int(b)) => Numeric::Int(a + b),
            (Numeric::Float(a), Numeric::Float(b)) => Numeric::Float(a + b),
            (self_unknown, other_unknown) => panic!("Cannot add numerics: {:?} + {:?}", self_unknown, other_unknown)
        }
    }
}

impl<'a> Sum<Numeric> for Numeric {
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = Numeric>, 
    {
        let mut int_vals: Vec<i64> = Vec::new();
        let mut float_vals: Vec<f64> = Vec::new();

        for val in iter {
            match val {
                Numeric::Int (x) => int_vals.push(x),
                Numeric::Float (x) => float_vals.push(x)
            }
        }

        if int_vals.len() > 0 && float_vals.len() > 0 {
            panic!("Cannot fold Numeric with float and ints");
        }

        if int_vals.len() > 0 {
            return Numeric::Int(int_vals.into_iter().sum());
        }

        if float_vals.len() > 0 {
            return Numeric::Float(float_vals.into_iter().sum())
        }

        panic!("Couldn't fold Numeric");
    }
}

#[derive(Debug, Clone)]
pub enum ExecuteOutput {
    // Array of ints
    ArrayOfNumerics(Vec<Numeric>),
    // TODO: is there a nicer way to make the inner value forced to Dict? perhaps the Dict(Dict) pattern with inner structure of enum
    // Array of dicts
    ArrayOfDicts(Vec<ExecuteOutput>),
    // Dict of string -> any value
    Dictionary(HashMap<String, ExecuteOutput>), 
    // Numeric int or float
    Numeric(Numeric),
    Null
}

#[derive(Debug, PartialEq, Clone)]
pub enum AstNode {
    Node(Box<AstNode>),
    Numeric(Numeric),
    DoublePrecisionFloat(f64),
    DyadicOp {
        verb: DyadicVerb,
        lhs: Box<AstNode>,
        rhs: Box<AstNode>
    },
    MonadicOp {
        verb: MonadicVerb,
        rhs: Box<AstNode>
    },
    OperatorOp {
        lhs_verb: DyadicVerb,
        operator_verb: OperatorVerb,
        rhs: Box<AstNode>
    },
    Terms(Vec<AstNode>),
    GlobalVar {
        variable: String,
        expression: Box<AstNode>,
    },
    Array(Vec<AstNode>),
    Dictionary(HashMap<String, AstNode>),
    Variable(String)
}

fn parse(source: &str) -> Result<Vec<AstNode>, Error<Rule>> {
    let mut ast = vec![];

    let pairs = ArrayLanguageParser::parse(Rule::program, source)?;
    for pair in pairs {
        match pair.as_rule() {
            Rule::expression => {
                ast.push(AstNode::Node(Box::new(build_ast_from_expr(pair))));
            }
            _ => {}
        }
    }

    Ok(ast)
}

fn build_ast_from_expr(pair: pest::iterators::Pair<Rule>) -> AstNode {
    match pair.as_rule() {
        Rule::expression => build_ast_from_expr(pair.into_inner().next().unwrap()),
        Rule::dyadicExpression => {
            let mut pair = pair.into_inner();

            let lhs = pair.next().unwrap();
            let lhs = build_ast_from_expr(lhs);

            let verb = pair.next().unwrap();
                    
            let rhs = pair.next().unwrap();
            let rhs = build_ast_from_expr(rhs);

            parse_dyadic_verb(lhs, verb, rhs)
        },
        Rule::monadicExpression => {
            let mut pair = pair.into_inner();

            let verb = pair.next().unwrap();
            let rhs = pair.next().unwrap();
            let rhs = build_ast_from_expr(rhs);

            parse_monadic_verb(verb, rhs)
        },
        Rule::operatorExpression => {
            let mut pair = pair.into_inner();

            let lhs_verb = pair.next().unwrap();
            
            let operator_verb = pair.next().unwrap();

            let rhs = pair.next().unwrap();
            let rhs = build_ast_from_expr(rhs);

            parse_operator_verb(lhs_verb, operator_verb, rhs)
        },
        Rule::assignment => {
            let mut pair = pair.into_inner();
            let variable = pair.next().unwrap();
            let expression = pair.next().unwrap();
            let expression = build_ast_from_expr(expression);
            AstNode::GlobalVar {
                variable: String::from(variable.as_str()),
                expression: Box::new(expression)
            }
        },
        Rule::terms => {
            let terms: Vec<AstNode> = pair.into_inner().map(build_ast_from_term).collect();

            // If single item, then unwrap it from vector
            match terms.len() {
                1 => terms.get(0).unwrap().clone(),
                _ => AstNode::Terms(terms)
            }
        },
        unknown_expr => panic!("Not implemented: {:?}", unknown_expr)
    }
}

fn build_ast_from_term(pair: pest::iterators::Pair<Rule>) -> AstNode {
    match pair.as_rule() {
        Rule::integer => {
            let istr = pair.as_str();
            let integer: i64 = istr.parse().unwrap();
            AstNode::Numeric(Numeric::Int(integer))
        },
        Rule::decimal => {
            let fstr = pair.as_str();
            let float: f64 = fstr.parse().unwrap();
            AstNode::Numeric(Numeric::Float(float))
        },
        Rule::array => {
            let vals: Vec<AstNode> = pair.into_inner().map(build_ast_from_term).collect();

            AstNode::Array(vals)
        },
        Rule::dictionary => {
            let mut dictionary: HashMap<String, AstNode> = HashMap::new();
            
            for entry in pair.into_inner() {
                let mut entry = entry.into_inner();

                // Get variable name, strip first and last char as they are "'" characters
                let mut var = entry.next().unwrap().as_str().chars();
                var.next();
                var.next_back();
                let var = var.as_str().to_string();

                let expr = entry.next().unwrap();
                let expr = build_ast_from_expr(expr);
                
                dictionary.insert(var, expr);
            }

            AstNode::Dictionary(dictionary)
        },
        Rule::expression => {
            build_ast_from_expr(pair)
        },
        Rule::variable => {
            AstNode::Variable(pair.as_str().to_string())
        },
        unknown_term => panic!("Unexpected term: {:?}", unknown_term)
    }
}

fn parse_dyadic_verb(lhs: AstNode, pair: pest::iterators::Pair<Rule>, rhs: AstNode) -> AstNode {
    AstNode::DyadicOp {
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
        verb: dyadic_verb_from_str(pair.as_str())
    }
}

fn dyadic_verb_from_str(verb_str: &str) -> DyadicVerb {
    match verb_str {
        "+" => DyadicVerb::Add,
        "/" => DyadicVerb::Replicate,
        ">" => DyadicVerb::GreaterThan,
        "÷" => DyadicVerb::Divide,
        other => panic!("Dyadic Verb {:?} not implemented", other)
    }
}

fn parse_monadic_verb(pair: pest::iterators::Pair<Rule>, rhs: AstNode) -> AstNode {
    AstNode::MonadicOp {
        rhs: Box::new(rhs),
        verb: match pair.as_str() {
            "print" => MonadicVerb::Print,
            "⍳" => MonadicVerb::Generate,
            "⍴" => MonadicVerb::Shape,
            other => panic!("Monadic Verb '{}' not implemented", other)
        }
    }
}

fn parse_operator_verb(lhs_verb_pair: pest::iterators::Pair<Rule>, operator_verb_pair: pest::iterators::Pair<Rule>, rhs: AstNode) -> AstNode {
    AstNode::OperatorOp {
        lhs_verb: dyadic_verb_from_str(lhs_verb_pair.as_str()),
        operator_verb: match operator_verb_pair.as_str() {
            "/" => OperatorVerb::Reduce,
            other => panic!("Operator Verb '{}' not implemented", other)
        },
        rhs: Box::new(rhs)
    }
}

fn run_program(program: Vec<AstNode>) {
    let mut state: HashMap<String, ExecuteOutput> = HashMap::new();

    for line in program {
        let node = match line {
            AstNode::Node(inner) => {
                *inner
            },
            node_matched => panic!("Unexpected node type in run program {:?}", node_matched)
        };

        // TODO: should we handle the assignment of variables out here instead? separately, then pass an immutable state in...
        execute_expression(node, &mut state);

        println!("State is: {:?}", state);
    }
}

fn execute_expression(expression: AstNode, state: &mut HashMap<String, ExecuteOutput>) -> ExecuteOutput {
    match expression {
        // Unwrap lhs and rhs and compute operation
        AstNode::DyadicOp {verb, lhs, rhs} => {
            let lhs = execute_expression(*lhs, state);
            let rhs = execute_expression(*rhs, state);
            execute_diadic_op(verb, lhs, rhs)
        },
        // Unwrap rhs and compute operation
        AstNode::MonadicOp {verb, rhs} => {
            let rhs = execute_expression(*rhs, state);
            execute_monadic_op(verb, rhs)
        },
        AstNode::OperatorOp {lhs_verb, operator_verb, rhs} => {
            let rhs = execute_expression(*rhs, state);
            execute_operator_op(lhs_verb, operator_verb, rhs)
        },
        // Unwrap + compute the inner values of the array
        AstNode::Array (vals) => {
            unwrap_array(vals, state)
        },
        // Fetch var from state and copy + return
        AstNode::Variable (var) => {
            unwrap_variable(var, state)
        },
        AstNode::Numeric (val) => {
            ExecuteOutput::Numeric(val)
        },
        AstNode::Dictionary (dict) => {
            unwrap_dictionary(dict, state)
        },
        AstNode::GlobalVar {variable, expression} => {
            // TODO: is this nice? it stops a mutable borrow of the state twice
            //        however what if the copy_state is updated inside execute_expression
            //        we shoudl deal with that here somehow?
            let mut copy_state = state.clone();
            state.insert(variable, execute_expression(*expression, &mut copy_state));

            // TODO Merge the inner and outer states? only needed once inner executions can modify state
            ExecuteOutput::Null
        },
        other_matched => panic!("Couldn't match node {:?} in execute expression", other_matched)
    }
}

fn execute_diadic_op(verb: DyadicVerb, lhs: ExecuteOutput, rhs: ExecuteOutput) -> ExecuteOutput {
    match verb {
        DyadicVerb::Add => {
            execute_add(lhs, rhs)
        },
        DyadicVerb::Divide => {
            execute_divide(lhs, rhs)
        },
        DyadicVerb::Replicate => {
            execute_replicate_array(lhs, rhs)
        },
        DyadicVerb::GreaterThan => {
            execute_array_greaterthan_int(lhs, rhs)
        },
        other => panic!("Dyadic verb not implemented {:?}", other)
    }
}

fn execute_monadic_op(verb: MonadicVerb, rhs: ExecuteOutput) -> ExecuteOutput {
    match verb {
        MonadicVerb::Print => {
            println!("PRINT {:?}", rhs);

            ExecuteOutput::Null
        },
        MonadicVerb::Generate => {
            let size = match rhs {
                ExecuteOutput::Numeric (Numeric::Int(int_val)) => int_val,
                other => panic!("Cant handle {:?} in monadic generate op", other)
            };

            let mut generated: Vec<Numeric> = Vec::new();

            for i in 0..size {
                generated.push(Numeric::Int(i as i64));
            }

            ExecuteOutput::ArrayOfNumerics(generated)
        },
        MonadicVerb::Shape => {
            // TODO: what should 'shape' of array of dicts and dicts be?
            let expression_size = match rhs {
                ExecuteOutput::ArrayOfNumerics (arr) => arr.len() as i64,
                ExecuteOutput::ArrayOfDicts (arr) => arr.len() as i64,
                other => panic!("Cant handle {:?} in monadic shape", other)
            };

            // TODO: replace this with an outupt of int or array depending on shape of object
            ExecuteOutput::Numeric(Numeric::Int(expression_size))
        }
    }
}

fn execute_operator_op(lhs_verb: DyadicVerb, operator_verb: OperatorVerb, rhs: ExecuteOutput) -> ExecuteOutput {
    match operator_verb {
        OperatorVerb::Reduce => {
            execute_reduce_dyadic_lhs(lhs_verb, rhs)
        },
        other => panic!("Operator verb not implemented {:?}", other)
    }
}

fn execute_reduce_dyadic_lhs(lhs_verb: DyadicVerb, rhs: ExecuteOutput) -> ExecuteOutput {
    match lhs_verb {
        DyadicVerb::Add => reduce_dyadic_add(rhs),
        other => panic!("Cannot reduce over dyadic verb: {:?}", other)
    }
}

fn reduce_dyadic_add(rhs: ExecuteOutput) -> ExecuteOutput {
    match rhs {
        ExecuteOutput::ArrayOfNumerics (arr) => {
            ExecuteOutput::Numeric(arr.into_iter().sum())
        },
        other => panic!("Cannot reduce add over {:?}", other)
    }
}

fn execute_add(lhs: ExecuteOutput, rhs: ExecuteOutput) -> ExecuteOutput {
    match (lhs, rhs) {
        // Adding two arrays of numbers
        (ExecuteOutput::ArrayOfNumerics (lhs_array), ExecuteOutput::ArrayOfNumerics (rhs_array)) => execute_add_int_arrays(lhs_array, rhs_array),
        // Adding an array + number
        (ExecuteOutput::ArrayOfNumerics (int_array), ExecuteOutput::Numeric (int_val))
            | (ExecuteOutput::Numeric (int_val), ExecuteOutput::ArrayOfNumerics(int_array)) => execute_add_int_array_to_int(int_array, int_val),
        // Adding two arrays of dicts
        (ExecuteOutput::ArrayOfDicts (lhs_array), ExecuteOutput::ArrayOfDicts (rhs_array)) => execute_add_array_of_dicts(lhs_array, rhs_array),
        // Adding two numbers
        (ExecuteOutput::Numeric (lhs_val), ExecuteOutput::Numeric (rhs_val)) => ExecuteOutput::Numeric(lhs_val + rhs_val),
        (lhs_other, rhs_other) => panic!("Cannot add pair ({:?}, {:?})", lhs_other, rhs_other)
    }
}

fn execute_divide(lhs: ExecuteOutput, rhs: ExecuteOutput) -> ExecuteOutput {
    match (lhs, rhs) {
        // Divide an array by an array
        (ExecuteOutput::ArrayOfNumerics (lhs_array), ExecuteOutput::ArrayOfNumerics (rhs_array)) => execute_divide_array_by_array(lhs_array, rhs_array),
        // Divide an array by a number
        (ExecuteOutput::ArrayOfNumerics (lhs_array), ExecuteOutput::Numeric (numeric)) => execute_divide_array_by_numeric(lhs_array, numeric),
        // Divide a number by a number
        (ExecuteOutput::Numeric (lhs_numeric), ExecuteOutput::Numeric (rhs_numeric)) => execute_divide_numeric_by_numeric(lhs_numeric, rhs_numeric),
        (lhs_other, rhs_other) => panic!("Cannot divide pair ({:?}, {:?})", lhs_other, rhs_other)
    }
}

fn execute_divide_array_by_numeric(lhs_array: Vec<Numeric>, numeric: Numeric) -> ExecuteOutput {
    let mut float_vals: Vec<f64> = Vec::new();

    for val in lhs_array {
        match val {
            Numeric::Float (x) => float_vals.push(x),
            Numeric::Int (x) => float_vals.push(x as f64)
        }
    }

    let denominator = match numeric {
        Numeric::Float(x) => x,
        Numeric::Int(x) => x as f64
    };

    let mut output: Vec<Numeric> = Vec::new();

    // TODO: is there a way to do this with float_vals.into_iter().map(...), couldn't get denominator to come into scope of lambda
    for val in float_vals {
        output.push(Numeric::Float(val / denominator));
    }

    ExecuteOutput::ArrayOfNumerics(output)
}

fn execute_divide_array_by_array(lhs_array: Vec<Numeric>, rhs_array: Vec<Numeric>) -> ExecuteOutput {
    if lhs_array.len() != rhs_array.len() {
        panic!("Cannot divide two arrays of different size {:?} vs {:?}", lhs_array, rhs_array);
    }

    let mut output: Vec<Numeric> = Vec::new();

    for i in 0..lhs_array.len() {
        let lhs_value = match *lhs_array.get(i).unwrap() {
            Numeric::Float (x) => x,
            Numeric::Int (x) => x as f64
        };

        let rhs_value = match *rhs_array.get(i).unwrap() {
            Numeric::Float (x) => x,
            Numeric::Int (x) => x as f64
        };

        output.push(Numeric::Float(lhs_value / rhs_value));
    }

    ExecuteOutput::ArrayOfNumerics(output)
}

fn execute_divide_numeric_by_numeric(lhs_numeric: Numeric, rhs_numeric: Numeric) -> ExecuteOutput {
    let lhs_float = match lhs_numeric {
        Numeric::Float(x) => x,
        Numeric::Int(x) => x as f64
    };

    let rhs_float = match rhs_numeric {
        Numeric::Float(x) => x,
        Numeric::Int(x) => x as f64
    };

    let result: f64 = lhs_float / rhs_float;

    ExecuteOutput::Numeric(Numeric::Float(result))
}

fn execute_add_int_arrays(lhs_array: Vec<Numeric>, rhs_array: Vec<Numeric>) -> ExecuteOutput {
    if lhs_array.len() != rhs_array.len() {
        panic!("Cannot add arrays of two different sizes: {} vs {}", lhs_array.len(), rhs_array.len());
    }

    println!("Adding arrays: {:?} + {:?}", lhs_array, rhs_array);

    let mut output: Vec<Numeric> = Vec::new();

    for i in 0..lhs_array.len() {
        output.push(*lhs_array.get(i).unwrap() + *rhs_array.get(i).unwrap());
    }

    println!("Result of addition: {:?}", output);
    ExecuteOutput::ArrayOfNumerics(output)
}

fn execute_add_array_of_dicts(lhs_array: Vec<ExecuteOutput>, rhs_array: Vec<ExecuteOutput>) -> ExecuteOutput {
    if lhs_array.len() != rhs_array.len() {
        panic!("Cannot add dict arrays of different lengths {:?} vs {:?}", lhs_array.len(), rhs_array.len());
    }

    let mut output: Vec<ExecuteOutput> = Vec::new();
    
    for i in 0..lhs_array.len() {
        let lhs_val = lhs_array.get(i).unwrap().clone();
        let rhs_val = rhs_array.get(i).unwrap().clone();

        match (lhs_val, rhs_val) {
            (ExecuteOutput::Dictionary (lhs_dict), ExecuteOutput::Dictionary (rhs_dict)) => { 
                let sum = add_dicts_of_numerics(lhs_dict, rhs_dict);
                output.push(ExecuteOutput::Dictionary(sum)); 
            },
            (lhs_other, rhs_other) => panic!("Cannot add dict array values {:?} vs {:?}", lhs_other, rhs_other)
        }
    }

    ExecuteOutput::ArrayOfDicts(output)
}

fn add_dicts_of_numerics(lhs_dict: HashMap<String, ExecuteOutput>, rhs_dict: HashMap<String, ExecuteOutput>) -> HashMap<String, ExecuteOutput> {
    if lhs_dict.len() != rhs_dict.len() {
        panic!("Cannot add dicts of different lengths: {:?} vs {:?}", lhs_dict.len(), rhs_dict.len());
    }

    let mut output: HashMap<String, ExecuteOutput> = HashMap::new();

    for (key, value) in lhs_dict {
        let rhs_value = rhs_dict.get(&key).unwrap().clone();
        output.insert(key, execute_add(value, rhs_value));
    }

    output
}

fn execute_add_int_array_to_int(int_array:  Vec<Numeric>, int_val: Numeric) -> ExecuteOutput {
    let output = int_array.into_iter().map(|x| x + int_val).collect();

    ExecuteOutput::ArrayOfNumerics(output)
}

fn execute_replicate_array(lhs: ExecuteOutput, rhs: ExecuteOutput) -> ExecuteOutput {
    let lhs_array: Vec<Numeric> = match lhs {
        ExecuteOutput::ArrayOfNumerics (arr) => arr,
        other => panic!("Array replication cant handle non array type {:?}", other)
    };

    let lhs_array: Vec<i64> = lhs_array.into_iter().map(|x| match x {
        Numeric::Int(x_int) => x_int,
        Numeric::Float(x_float) => panic!("Cannot replicate with float values on lhs {:?}", x_float)
    }).collect();


    match rhs {
        ExecuteOutput::ArrayOfNumerics (arr) => execute_replicate_array_of_numerics(lhs_array, arr),
        ExecuteOutput::ArrayOfDicts (arr) => execute_replicate_array_of_dicts(lhs_array, arr),
        other => panic!("Array replication cant handle non array type {:?}", other)
    }
}

fn execute_replicate_array_of_numerics(lhs_array: Vec<i64>, rhs_array: Vec<Numeric>) -> ExecuteOutput {
    if lhs_array.len() != rhs_array.len() {
        panic!("Cannot add arrays of two different sizes: {} vs {}", lhs_array.len(), rhs_array.len());
    }

    println!("Replicating: {:?} / {:?}", lhs_array, rhs_array);

    let mut output = Vec::new();

    for i in 0..lhs_array.len() {
        let multiplicity = lhs_array.get(i).unwrap();

        if *multiplicity < 0 {
            panic!("Multiplicity {} is less than zero, not allowed in replicate command", multiplicity);
        }

        for _ in 0..*multiplicity {
            output.push(rhs_array.get(i).unwrap().clone());
        }
    }

    ExecuteOutput::ArrayOfNumerics(output)
}

fn execute_replicate_array_of_dicts(lhs_array: Vec<i64>, rhs_array: Vec<ExecuteOutput>) -> ExecuteOutput {
    if lhs_array.len() != rhs_array.len() {
        panic!("Cannot add arrays of two different sizes: {} vs {}", lhs_array.len(), rhs_array.len());
    }

    println!("Replicating: {:?} / {:?}", lhs_array, rhs_array);

    let mut output = Vec::new();

    for i in 0..lhs_array.len() {
        let multiplicity = lhs_array.get(i).unwrap();

        if *multiplicity < 0 {
            panic!("Multiplicity {} is less than zero, not allowed in replicate command", multiplicity);
        }

        for _ in 0..*multiplicity {
            output.push(rhs_array.get(i).unwrap().clone());
        }
    }

    ExecuteOutput::ArrayOfDicts(output)
}

fn execute_array_greaterthan_int(lhs: ExecuteOutput, rhs: ExecuteOutput) -> ExecuteOutput {
    let lhs_array = match lhs {
        ExecuteOutput::ArrayOfNumerics (arr) => arr,
        other => panic!("Array greaterthan cant handle non array lhs type {:?}", other)
    };

    let rhs_integer = match rhs {
        ExecuteOutput::Numeric (val) => val,
        other => panic!("Array greaterthan cant handle non Numeric rhs type {:?}", other)
    };

    let mut output: Vec<Numeric> = Vec::new();

    for val in lhs_array {
        if val > rhs_integer {
            output.push(Numeric::Int(1))
        } else {
            output.push(Numeric::Int(0))
        }
    }

    ExecuteOutput::ArrayOfNumerics(output)
}

fn unwrap_array(vals: Vec<AstNode>, state: &mut HashMap<String, ExecuteOutput>) -> ExecuteOutput {
    let vals_clone = vals.clone();
    let mut int_array: Vec<Numeric> = Vec::new();
    let mut float_array: Vec<Numeric> = Vec::new();
    let mut dict_array: Vec<ExecuteOutput> = Vec::new();

    for val in vals {
        match val {
            AstNode::Numeric (Numeric::Int(int_val))  => {
                int_array.push(Numeric::Int(int_val))
            },
            AstNode::Numeric (Numeric::Float(float_val)) => {
                float_array.push(Numeric::Float(float_val))
            },
            AstNode::Dictionary (dict_val) => {
                dict_array.push(unwrap_dictionary(dict_val, state))
            }
            other => panic!("cant handle array of: {:?}", other)
        };
    };

    let sizes = vec![int_array.len(), float_array.len(), dict_array.len()];
    let nonzero_sizes: Vec<usize> = sizes.into_iter().filter(|size| size > &0).collect();

    if nonzero_sizes.len() > 1 {
        panic!("Cannot have mixed array: {:?}", vals_clone);
    }

    if int_array.len() > 0 {
        return ExecuteOutput::ArrayOfNumerics(int_array);
    }

    if float_array.len() > 0 {
        return ExecuteOutput::ArrayOfNumerics(float_array);
    }

    // TODO: what if both arrays are size zero? then we return dict array by default?
    return ExecuteOutput::ArrayOfDicts(dict_array)
}

fn unwrap_dictionary(dict: HashMap<String, AstNode>, state: &mut HashMap<String, ExecuteOutput>) -> ExecuteOutput {
    let mut unwrapped_dict: HashMap<String, ExecuteOutput> = HashMap::new();

    for (key, value) in dict {
        unwrapped_dict.insert(key, execute_expression(value, state));
    }

    ExecuteOutput::Dictionary(unwrapped_dict)
}

// Given a variable name, unwrap its value, copy the data from state and return a new execute output
fn unwrap_variable(var: String, state: &HashMap<String, ExecuteOutput>) -> ExecuteOutput {
    let var_value = match state.get(&var).unwrap() {
        ExecuteOutput::ArrayOfNumerics (arr) => {
            let mut copied_var: Vec<Numeric> = Vec::new();
            for val in arr {
                copied_var.push(*val);
            }

            ExecuteOutput::ArrayOfNumerics(copied_var)
        },
        ExecuteOutput::ArrayOfDicts (dict) => {
            ExecuteOutput::ArrayOfDicts(dict.clone())
        },
        ExecuteOutput::Dictionary (dict) => {
            let mut copied_dict: HashMap<String, ExecuteOutput> = HashMap::new();

            for (key, values) in dict {
                copied_dict.insert(key.to_string(), values.clone());
            }

            ExecuteOutput::Dictionary(copied_dict)
        },
        ExecuteOutput::Numeric (int_val) => {
            ExecuteOutput::Numeric(int_val.clone())
        },
        other => panic!("Cant handle variables of type {:?}", other)
    };

    var_value
}

fn main() {
    let unparsed_file = fs::read_to_string("test_program_2.txt").expect("cannot read file");

    println!("Raw file:\n{:?}", unparsed_file);

    let file = ArrayLanguageParser::parse(Rule::program, &unparsed_file)
        .expect("unsuccessful parse")
        .next().unwrap();

    println!("{:?}", file);

    let out = parse(&unparsed_file).unwrap();

    println!("{:?}", out);

    run_program(out);
}

